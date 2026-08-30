//! Code generation for validated configuration declarations.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::LitStr;

use super::input::{Config, DefaultValue, option_inner, vec_inner};

/// Expands a validated configuration declaration.
pub(crate) fn generate_config(
    config: &Config,
    argx: &TokenStream2,
    serde: &LitStr,
) -> TokenStream2 {
    let ident = &config.ident;
    let visibility = &config.visibility;
    let type_name = ident.to_string();
    let type_name = type_name.strip_prefix("r#").unwrap_or(&type_name);
    let overrides_ident = format_ident!("__ArgxOverridesFor{}", type_name);
    let toml_ident = format_ident!("__ArgxTomlFor{}", type_name);
    let cli_ident = format_ident!("__ArgxCliFor{}", type_name);
    let parser_ident = format_ident!("__ArgxConfigParserFor{}", type_name);

    let cli_fields = config.fields.iter().filter(|field| field.exposed_on_cli()).map(|field| {
        let ident = &field.ident;
        let docs = &field.docs;
        if field.nested {
            let ty = &field.ty;
            return quote! {
                #(#docs)*
                #[argx(flatten)]
                #ident: <#ty as #argx::__private::Config>::__CliArgs
            };
        }
        let attrs = &field.cli;
        let ty = &field.ty;
        let cli_ty = if field.optional {
            let inner = option_inner(ty).expect("optional field has Option inner type");
            quote!(::core::option::Option<#inner>)
        } else if field.many {
            quote!(#ty)
        } else {
            quote!(::core::option::Option<#ty>)
        };
        quote! {
            #(#docs)*
            #[argx(#(#attrs),*)]
            #ident: #cli_ty
        }
    });

    let cli_overrides = config.fields.iter().filter(|field| field.exposed_on_cli()).map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            return quote! {
                overrides.#ident = ::core::option::Option::Some(
                    <#ty as #argx::__private::Config>::__cli_overrides(args.#ident)
                );
            };
        }
        if field.many {
            return quote! {
                if !args.#ident.is_empty() {
                    overrides.#ident = ::core::option::Option::Some(args.#ident);
                }
            };
        }
        quote! {
            if let ::core::option::Option::Some(value) = args.#ident {
                overrides.#ident = ::core::option::Option::Some(value);
            }
        }
    });

    let override_fields = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            quote!(#ident: ::core::option::Option<<#ty as #argx::__private::Config>::Overrides>)
        } else {
            quote!(#ident: ::core::option::Option<#ty>)
        }
    });
    let toml_fields = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            quote!(#ident: ::core::option::Option<<#ty as #argx::__private::Config>::__Toml>)
        } else {
            quote!(#ident: ::core::option::Option<#ty>)
        }
    });
    let merges = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            quote! {
                if let ::core::option::Option::Some(higher) = higher.#ident {
                    lower.#ident = ::core::option::Option::Some(match lower.#ident.take() {
                        ::core::option::Option::Some(mut nested_lower) => {
                            <#ty as #argx::__private::Config>::__merge(&mut nested_lower, higher);
                            nested_lower
                        }
                        ::core::option::Option::None => higher,
                    });
                }
            }
        } else {
            quote! {
                if let ::core::option::Option::Some(value) = higher.#ident {
                    lower.#ident = ::core::option::Option::Some(value);
                }
            }
        }
    });
    let defaults = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            return quote! {
                #ident: ::core::option::Option::Some(<#ty as #argx::__private::Config>::__defaults())
            };
        }
        match &field.default {
            Some(DefaultValue::Trait) => quote! {
                #ident: ::core::option::Option::Some(<#ty as ::core::default::Default>::default())
            },
            Some(DefaultValue::Expression(expression)) => quote! {
                #ident: ::core::option::Option::Some({
                    let value: #ty = #expression;
                    value
                })
            },
            None => quote!(#ident: ::core::option::Option::None),
        }
    });
    let toml_into_overrides = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if field.nested {
            quote! {
                #ident: input.#ident.map(
                    <#ty as #argx::__private::Config>::__toml_overrides,
                )
            }
        } else {
            quote!(#ident: input.#ident)
        }
    });

    let own_prefix = config.prefix.as_ref().map_or_else(
        || quote!(inherited_prefix),
        |prefix| quote!(inherited_prefix.or(::core::option::Option::Some(#prefix))),
    );
    let environment_contract_fields = config.fields.iter().map(|field| {
        let ty = &field.ty;
        let field_name = LitStr::new(&field.name, field.ident.span());
        if field.nested {
            let component = LitStr::new(&field.name.to_ascii_uppercase(), field.ident.span());
            return quote! {
                let nested_prefix = prefix
                    .map(|prefix| #argx::__private::environment_name(prefix, #component));
                let nested = <#ty as #argx::__private::Config>::__environment_contract(
                    nested_prefix.as_deref(),
                );
                contract.__extend_within(nested, #field_name);
            };
        }

        field.env.as_ref().map_or_else(
            || {
                let component = LitStr::new(&field.name.to_ascii_uppercase(), field.ident.span());
                quote! {
                    if let ::core::option::Option::Some(prefix) = prefix {
                        contract.__binding(
                            #field_name,
                            #argx::__private::environment_name(prefix, #component),
                        );
                    }
                }
            },
            |environment| {
                quote! {
                    contract.__binding(#field_name, #environment);
                }
            },
        )
    });

    let environment_fields = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let field_name = LitStr::new(&field.name, field.ident.span());
        if field.nested {
            let component = LitStr::new(&field.name.to_ascii_uppercase(), field.ident.span());
            return quote! {
                let nested_prefix = prefix
                    .map(|prefix| #argx::__private::environment_name(prefix, #component));
                overrides.#ident = ::core::option::Option::Some(
                    <#ty as #argx::__private::Config>::__environment_with_prefix(
                        environment,
                        nested_prefix.as_deref(),
                    )
                    .map_err(|error| error.__within(#field_name))?,
                );
            };
        }

        let parser = if field.delimited {
            let inner = vec_inner(ty)
                .or_else(|| option_inner(ty).and_then(vec_inner))
                .expect("delimited Config field is validated as a collection by Args derive");
            quote!(#argx::__private::parse_environment_delimited_field::<#inner>)
        } else {
            quote!(#argx::__private::parse_environment_field::<#ty>)
        };
        field.env.as_ref().map_or_else(|| {
            let component = LitStr::new(&field.name.to_ascii_uppercase(), field.ident.span());
            quote! {
                if let ::core::option::Option::Some(prefix) = prefix {
                    let environment_name = #argx::__private::environment_name(prefix, #component);
                    if let ::core::option::Option::Some(value) =
                        #parser(environment, #field_name, &environment_name)?
                    {
                        overrides.#ident = ::core::option::Option::Some(value);
                    }
                }
            }
        }, |environment| quote! {
                if let ::core::option::Option::Some(value) =
                    #parser(environment, #field_name, #environment)?
                {
                    overrides.#ident = ::core::option::Option::Some(value);
                }
            })
    });

    let discard_higher = config.fields.is_empty().then(|| quote!(let _ = higher;));
    let discard_resolved = config.fields.is_empty().then(|| quote!(let _ = resolved;));
    let resolved_fields = config.fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let name = LitStr::new(&field.name, field.ident.span());
        if field.nested {
            return quote! {
                #ident: <#ty as #argx::__private::Config>::__finalize(
                    resolved.#ident.unwrap_or_default(),
                )
                .map_err(|error| error.__within(#name))?
            };
        }
        if field.optional && field.default.is_none() {
            quote!(#ident: resolved.#ident.unwrap_or_default())
        } else {
            quote! {
                #ident: match resolved.#ident {
                    ::core::option::Option::Some(value) => value,
                    ::core::option::Option::None => {
                        return ::core::result::Result::Err(#argx::config::SourceError::__missing_value(#name));
                    }
                }
            }
        }
    });

    quote! {
        impl #ident {
            /// Creates an empty ordered configuration loader.
            pub fn loader() -> #argx::config::Loader<Self> {
                #argx::config::Loader::default()
            }
        }

        #[doc(hidden)]
        #[derive(::core::default::Default)]
        #[must_use = "configuration overrides have no effect until they are resolved"]
        #visibility struct #overrides_ident {
            #(#override_fields,)*
        }

        impl ::core::fmt::Debug for #overrides_ident {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(::core::stringify!(#overrides_ident))
            }
        }

        #[doc(hidden)]
        #[derive(#argx::Args)]
        #visibility struct #cli_ident {
            #(#cli_fields,)*
        }

        #[doc(hidden)]
        #[derive(#argx::Parser)]
        struct #parser_ident {
            #[argx(flatten)]
            values: #cli_ident,
        }

        #[doc(hidden)]
        #[derive(#argx::__private::serde::Deserialize)]
        #[serde(crate = #serde, deny_unknown_fields)]
        #visibility struct #toml_ident {
            #(
                #[serde(default)]
                #toml_fields,
            )*
        }

        impl ::core::fmt::Debug for #toml_ident {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(::core::stringify!(#toml_ident))
            }
        }

        impl #argx::__private::Config for #ident {
            type Overrides = #overrides_ident;
            type __Toml = #toml_ident;
            type __CliArgs = #cli_ident;

            fn __parse_cli<I, T>(argv: I) -> ::core::result::Result<Self::Overrides, #argx::Error>
            where
                I: ::core::iter::IntoIterator<Item = T>,
                T: ::core::convert::Into<::std::ffi::OsString>,
            {
                use #argx::Parser as _;
                let parsed = #parser_ident::try_parse_from(argv)?;
                ::core::result::Result::Ok(Self::__cli_overrides(parsed.values))
            }

            fn __cli_overrides(args: Self::__CliArgs) -> Self::Overrides {
                let mut overrides = <#overrides_ident as ::core::default::Default>::default();
                #(#cli_overrides)*
                overrides
            }

            fn __merge(lower: &mut Self::Overrides, higher: Self::Overrides) {
                #discard_higher
                #(#merges)*
            }

            fn __toml_overrides(input: Self::__Toml) -> Self::Overrides {
                #overrides_ident {
                    #(#toml_into_overrides,)*
                }
            }

            fn __defaults() -> Self::Overrides {
                #overrides_ident {
                    #(#defaults,)*
                }
            }

            fn __environment_contract(
                inherited_prefix: ::core::option::Option<&str>,
            ) -> #argx::__private::EnvironmentContract {
                let prefix = #own_prefix;
                let mut contract = #argx::__private::EnvironmentContract::default();
                #(#environment_contract_fields)*
                contract
            }

            fn __environment_with_prefix(
                environment: &#argx::__private::Environment,
                inherited_prefix: ::core::option::Option<&str>,
            ) -> ::core::result::Result<Self::Overrides, #argx::__private::EnvironmentError> {
                let prefix = #own_prefix;
                let mut overrides = <#overrides_ident as ::core::default::Default>::default();
                let _ = &mut overrides;
                let _ = environment;
                let _ = prefix;
                #(#environment_fields)*
                ::core::result::Result::Ok(overrides)
            }

            fn __finalize(resolved: Self::Overrides) -> ::core::result::Result<Self, #argx::config::SourceError> {
                #discard_resolved
                ::core::result::Result::Ok(Self {
                    #(#resolved_fields,)*
                })
            }
        }
    }
}
