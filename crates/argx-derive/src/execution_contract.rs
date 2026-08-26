//! Execution-contract attribute expansion.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, ItemFn, Meta, ReturnType, Token, Type,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};

use crate::crate_name;

/// Parsed arguments to `#[argx::contract(...)]`.
struct ContractArguments {
    /// Invocable command identity receiving this contract.
    command_type: Type,
}

impl Parse for ContractArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "#[argx::contract] requires an invocable command type, for example #[argx::contract(GetArgs)]",
            ));
        }

        let command_type = input.parse()?;
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error(
                "unsupported Argx execution contract arguments; expected only #[argx::contract(CommandType)]",
            ));
        }

        Ok(Self { command_type })
    }
}

/// Expands one canonical command execution contract around a free handler function.
pub(crate) fn contract(attribute: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let arguments = syn::parse2::<ContractArguments>(attribute)?;
    let function = syn::parse2::<ItemFn>(input).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "#[argx::contract(CommandType)] can only be applied to a free function",
        )
    })?;

    validate_signature(&function)?;
    validate_conditional_attributes(&function.attrs)?;
    let output = match &function.sig.output {
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &function.sig,
                "Argx execution contracts require a concrete Result<Success, Error> return type",
            ));
        }
        ReturnType::Type(_, output) => output.as_ref(),
    };
    if matches!(output, Type::ImplTrait(_)) {
        return Err(syn::Error::new_spanned(
            output,
            "Argx execution contracts do not support opaque `impl Trait` return types",
        ));
    }

    let facade = crate_name::facade_path();
    let command_type = arguments.command_type;
    let resolution = quote! {
        <#output as #facade::__private::ExecutionResult>::resolve_execution(resolver)
    };
    let conditional = function
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .collect::<Vec<_>>();

    Ok(quote! {
        #function

        #(#conditional)*
        impl #facade::__private::ExecutionContractSource for #command_type {
            fn resolve_execution(
                resolver: &mut #facade::__private::TypeResolver,
            ) -> #facade::ExecutionContract {
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
                "Argx execution contract handlers cannot use `cfg_attr` to add `cfg`; use an explicit `cfg` attribute",
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

/// Rejects handler signatures that cannot name one concrete execution result contract.
fn validate_signature(function: &ItemFn) -> syn::Result<()> {
    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "Argx execution contract handlers must be non-generic",
        ));
    }
    if function.sig.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "Argx execution contract handlers do not support variadic parameters",
        ));
    }
    Ok(())
}
