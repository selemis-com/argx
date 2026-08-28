//! Handler attribute expansion.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, ImplItem, Item, ItemFn, ItemImpl, Meta, ReturnType, Token, Type,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};

use crate::crate_name;

/// Parsed arguments to the free-function handler form.
struct HandlerArguments {
    /// Invocable command identity receiving this handler.
    command_type: Type,
}

impl Parse for HandlerArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[argx::handler] requires a command type or inherent handler method",
            ));
        }

        let command_type = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(
                input.error("unsupported Argx handler arguments; expected one command type")
            );
        }

        Ok(Self { command_type })
    }
}

/// Expands one canonical command handler around a free function or inherent implementation.
pub(crate) fn handler(attribute: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attribute_span =
        attribute.clone().into_iter().next().map_or_else(Span::call_site, |token| token.span());

    match syn::parse2::<Item>(input).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "#[argx::handler(...)] can only be applied to a free function or inherent impl",
        )
    })? {
        Item::Fn(function) => free_handler(attribute, &function),
        Item::Impl(item_impl) => impl_handler(attribute, &item_impl),
        _ => Err(syn::Error::new(
            attribute_span,
            "#[argx::handler(...)] can only be applied to a free function or inherent impl",
        )),
    }
}

/// Expands `#[argx::handler(CommandType)] fn ...`.
fn free_handler(attribute: TokenStream, function: &ItemFn) -> syn::Result<TokenStream> {
    let arguments = syn::parse2::<HandlerArguments>(attribute)?;
    validate_signature(&function.sig)?;
    validate_conditional_attributes(&function.attrs)?;
    let output = concrete_output(&function.sig)?;
    let conditional = cfg_attributes(&function.attrs);
    let item = quote!(#function);
    expand_association(&item, &arguments.command_type, &output, &conditional)
}

/// Expands `#[argx::handler(method)] impl CommandType { ... }`.
fn impl_handler(attribute: TokenStream, item_impl: &ItemImpl) -> syn::Result<TokenStream> {
    let method = syn::parse2::<syn::Ident>(attribute).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "handler impls require one method name, for example #[argx::handler(run)]",
        )
    })?;
    if item_impl.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            &item_impl.self_ty,
            "Argx handlers must annotate an inherent impl",
        ));
    }
    if !item_impl.generics.params.is_empty() || item_impl.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &item_impl.generics,
            "Argx handler impls must be non-generic",
        ));
    }

    validate_conditional_attributes(&item_impl.attrs)?;
    let selected = item_impl
        .items
        .iter()
        .find_map(|item| match item {
            ImplItem::Fn(function) if function.sig.ident == method => Some(function),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &item_impl.self_ty,
                format!("Argx handler method `{method}` was not found in this impl"),
            )
        })?;
    validate_signature(&selected.sig)?;
    validate_conditional_attributes(&selected.attrs)?;
    let output = concrete_output(&selected.sig)?;
    let mut conditional = cfg_attributes(&item_impl.attrs);
    conditional.extend(cfg_attributes(&selected.attrs));
    let item = quote!(#item_impl);

    expand_association(&item, &item_impl.self_ty, &output, &conditional)
}

/// Emits one handler item plus the static schema association for its command type.
fn expand_association(
    item: &TokenStream,
    command_type: &Type,
    output: &Type,
    conditional: &[Attribute],
) -> syn::Result<TokenStream> {
    let facade = crate_name::facade_path();
    let resolution = quote! {
        <#output as #facade::__private::HandlerResult>::schemas()
    };

    Ok(quote! {
        #item

        #(#conditional)*
        impl #facade::HandlerSchemaSource for #command_type {
            fn handler_schemas() -> #facade::__private::HandlerSchemas {
                #resolution
            }
        }

        #(#conditional)*
        impl #facade::__private::SchemaCommand for #command_type {
            fn register_schema_commands(
                command: &'static #facade::__private::Command<'static>,
                registry: &mut #facade::__private::SchemaRegistry,
            ) {
                #facade::__private::register_schema_handler::<Self>(command, registry);
            }
        }
    })
}

/// Returns one concrete handler return type.
fn concrete_output(signature: &syn::Signature) -> syn::Result<Type> {
    let output = match &signature.output {
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                signature,
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
    Ok(output.clone())
}

/// Extracts explicit `cfg` attributes that must also guard generated associations.
fn cfg_attributes(attributes: &[Attribute]) -> Vec<Attribute> {
    attributes.iter().filter(|attribute| attribute.path().is_ident("cfg")).cloned().collect()
}

/// Rejects conditional attribute forms that cannot be safely mirrored onto generated impls.
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
fn validate_signature(signature: &syn::Signature) -> syn::Result<()> {
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "Argx handlers must be non-generic",
        ));
    }
    if signature.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            signature,
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
            !cfg_attr_controls_presence(&unrelated).expect("unrelated metadata should be safe")
        );

        let no_attributes: Attribute = parse_quote!(#[cfg_attr(feature = "extra")]);
        assert!(
            !cfg_attr_controls_presence(&no_attributes).expect("empty cfg_attr should be safe")
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
                .expect("nested unrelated metadata should be safe")
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
