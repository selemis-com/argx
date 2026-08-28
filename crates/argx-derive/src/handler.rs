//! Handler attribute expansion.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, ItemFn, Meta, ReturnType, Token, Type,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};

use crate::crate_name;

/// Parsed arguments to `#[argx::handler(...)]`.
struct HandlerArguments {
    /// Invocable command identity receiving this handler.
    command_type: Type,
}

impl Parse for HandlerArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[argx::handler] requires an invocable command type, for example #[argx::handler(GetArgs)]",
            ));
        }

        let command_type = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error(
                "unsupported Argx handler arguments; expected only #[argx::handler(CommandType)]",
            ));
        }

        Ok(Self { command_type })
    }
}

/// Expands one canonical command handler around a free handler function.
pub(crate) fn handler(attribute: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let arguments = syn::parse2::<HandlerArguments>(attribute)?;
    let function = syn::parse2::<ItemFn>(input).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "#[argx::handler(CommandType)] can only be applied to a free function",
        )
    })?;

    validate_signature(&function)?;
    validate_conditional_attributes(&function.attrs)?;
    let output = match &function.sig.output {
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &function.sig,
                "Argx handlers require a concrete Result<Success, Error> return type",
            ));
        }
        ReturnType::Type(_, output) => output.as_ref(),
    };
    if matches!(output, Type::ImplTrait(_)) {
        return Err(syn::Error::new_spanned(
            output,
            "Argx handlers do not support opaque `impl Trait` return types",
        ));
    }

    let facade = crate_name::facade_path();
    let command_type = arguments.command_type;
    let resolution = quote! {
        <#output as #facade::__private::HandlerResult>::schemas()
    };
    let conditional = function
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .collect::<Vec<_>>();

    Ok(quote! {
        #function

        #(#conditional)*
        impl #facade::HandlerSchemaSource for #command_type {
            fn handler_schemas() -> #facade::__private::HandlerSchemas {
                #resolution
            }
        }
    })
}

/// Rejects conditional attribute forms that cannot be safely mirrored onto the generated impl.
fn validate_conditional_attributes(attributes: &[Attribute]) -> syn::Result<()> {
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("cfg_attr")) {
        if cfg_attr_controls_presence(attribute)? {
            return Err(syn::Error::new_spanned(
                attribute,
                "Argx handlers cannot use `cfg_attr` to add `cfg`; use an explicit `cfg` attribute",
            ));
        }
    }
    Ok(())
}

/// Reports whether one `cfg_attr` can remove the handler from the compilation unit.
fn cfg_attr_controls_presence(attribute: &Attribute) -> syn::Result<bool> {
    let arguments = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    for meta in arguments.iter().skip(1) {
        if meta_controls_presence(meta)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Recursively detects `cfg` inside the attribute portion of a `cfg_attr`.
fn meta_controls_presence(meta: &Meta) -> syn::Result<bool> {
    if meta.path().is_ident("cfg") {
        return Ok(true);
    }
    let Meta::List(list) = meta else {
        return Ok(false);
    };
    if !list.path.is_ident("cfg_attr") {
        return Ok(false);
    }

    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
    for meta in arguments.iter().skip(1) {
        if meta_controls_presence(meta)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Rejects handler signatures that cannot name one concrete result type.
fn validate_signature(function: &ItemFn) -> syn::Result<()> {
    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "Argx handlers must be non-generic",
        ));
    }
    if function.sig.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "Argx handlers do not support variadic parameters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{Attribute, Meta, parse_quote};

    use super::{
        cfg_attr_controls_presence, handler, meta_controls_presence,
        validate_conditional_attributes,
    };

    #[test]
    fn cfg_attr_presence_detection_handles_direct_nested_and_unrelated_attributes() {
        let direct: Attribute = parse_quote!(#[cfg_attr(feature = "extra", cfg(unix))]);
        assert!(cfg_attr_controls_presence(&direct).expect("direct cfg should be detected"));

        let nested: Attribute =
            parse_quote!(#[cfg_attr(feature = "extra", cfg_attr(unix, cfg(target_os = "linux")))]);
        assert!(cfg_attr_controls_presence(&nested).expect("nested cfg should be detected"));

        let unrelated: Attribute = parse_quote!(#[cfg_attr(feature = "extra", allow(dead_code))]);
        assert!(
            !cfg_attr_controls_presence(&unrelated).expect("unrelated metadata should be safe"),
        );

        let no_attributes: Attribute = parse_quote!(#[cfg_attr(feature = "extra")]);
        assert!(
            !cfg_attr_controls_presence(&no_attributes).expect("empty cfg_attr should be safe"),
        );
    }

    #[test]
    fn recursive_presence_detection_ignores_non_cfg_metadata() {
        let path: Meta = parse_quote!(allow);
        assert!(!meta_controls_presence(&path).expect("plain metadata should be safe"));

        let unrelated_list: Meta = parse_quote!(allow(dead_code));
        assert!(!meta_controls_presence(&unrelated_list).expect("unrelated lists should be safe"));

        let nested_safe: Meta = parse_quote!(cfg_attr(unix, allow(dead_code)));
        assert!(
            !meta_controls_presence(&nested_safe)
                .expect("nested unrelated metadata should be safe"),
        );
    }

    #[test]
    fn handler_rejects_cfg_attr_presence_control_before_codegen() {
        let error = handler(
            quote!(ConditionalPresence),
            quote! {
                #[cfg_attr(feature = "extra", cfg(unix))]
                fn conditional_presence() -> Result<(), ()> {
                    Ok(())
                }
            },
        )
        .expect_err("cfg_attr adding cfg must be rejected before code generation");
        assert_eq!(
            error.to_string(),
            "Argx handlers cannot use `cfg_attr` to add `cfg`; use an explicit `cfg` attribute",
        );
    }

    #[test]
    fn conditional_attribute_validation_rejects_presence_control() {
        let attributes = [
            parse_quote!(#[allow(dead_code)]),
            parse_quote!(#[cfg_attr(feature = "extra", cfg(unix))]),
        ];
        let error = validate_conditional_attributes(&attributes)
            .expect_err("cfg_attr adding cfg must be rejected");
        assert_eq!(
            error.to_string(),
            "Argx handlers cannot use `cfg_attr` to add `cfg`; use an explicit `cfg` attribute",
        );
    }
}
