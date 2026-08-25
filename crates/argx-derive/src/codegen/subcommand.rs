//! Code generation for `Subcommand` enums.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Generics, parse_quote};

use super::option_str;
use crate::{crate_name, key, model};

/// Generates static child-command tables and typed enum binding.
pub(crate) fn subcommands(subcommand: &model::Subcommand) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &subcommand.ident;
    let generics = subcommand_generics(subcommand, &facade);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let keys =
        key::subcommand_constants(&facade, &subcommand.fingerprint, subcommand.variants.len());

    let command_tables = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let table = format_ident!("ARGX_SUBCOMMAND_{index}");
        let key = key::ident("SUBCOMMAND", Some(index));
        let name = &variant.name;
        let about = option_str(variant.about.as_deref());
        variant.payload.as_ref().map_or_else(|| quote! {
                static #table: #facade::__private::Command<'static> =
                    #facade::__private::Command {
                        name: #name,
                        about: #about,
                        flags: &[],
                        args: &[],
                        subcommands: &[],
                        key: #key,
                    };
            }, |ty| quote! {
                static #table: #facade::__private::Command<'static> =
                    #facade::__private::Command {
                        name: #name,
                        about: #about,
                        flags: <#ty as #facade::__private::CommandArgs>::COMMAND.flags,
                        args: <#ty as #facade::__private::CommandArgs>::COMMAND.args,
                        subcommands: <#ty as #facade::__private::CommandArgs>::COMMAND.subcommands,
                        key: #key,
                    };
            })
    });
    let command_refs = (0..subcommand.variants.len()).map(|index| {
        let table = format_ident!("ARGX_SUBCOMMAND_{index}");
        quote!(&#table)
    });

    // Only one sibling command can be active. An enum keeps the accumulator proportional to the
    // largest selected branch instead of reserving space for every sibling's partial state.
    let partial_variants = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        variant.payload.as_ref().map_or_else(
            || quote!(#partial_variant,),
            |ty| quote!(#partial_variant(<#ty as #facade::__private::CommandArgs>::Partial),),
        )
    });

    let selected_apply_arms = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        variant.payload.as_ref().map_or_else(
            || quote!(Partial::#partial_variant => return false,),
            |ty| {
                quote! {
                    Partial::#partial_variant(selected) => {
                        return <#ty as #facade::__private::CommandArgs>::apply(selected, event);
                    },
                }
            },
        )
    });
    let select_arms = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let key = key::ident("SUBCOMMAND", Some(index));
        let partial_variant = format_ident!("V{index}");
        variant.payload.as_ref().map_or_else(
            || {
                quote! {
                    #key => {
                        *partial = Partial::#partial_variant;
                        true
                    }
                }
            },
            |ty| {
                quote! {
                    #key => {
                        *partial = Partial::#partial_variant(
                            <#ty as #facade::__private::CommandArgs>::start(),
                        );
                        true
                    }
                }
            },
        )
    });

    let occurrence_arms = subcommand
        .variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| {
            let ty = variant.payload.as_ref()?;
            let partial_variant = format_ident!("V{index}");
            Some(quote! {
                Partial::#partial_variant(selected) => {
                    <#ty as #facade::__private::CommandArgs>::check_occurrences(selected)
                }
            })
        })
        .collect::<Vec<_>>();
    let required_arms = subcommand
        .variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| {
            let ty = variant.payload.as_ref()?;
            let partial_variant = format_ident!("V{index}");
            Some(quote! {
                Partial::#partial_variant(selected) => {
                    <#ty as #facade::__private::CommandArgs>::check_required(selected)
                }
            })
        })
        .collect::<Vec<_>>();
    let occurrence_partial =
        if occurrence_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
    let required_partial =
        if required_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
    let occurrence_body = if occurrence_arms.is_empty() {
        quote!(::std::result::Result::Ok(()))
    } else {
        quote! {
            match partial {
                #(#occurrence_arms,)*
                _ => ::std::result::Result::Ok(()),
            }
        }
    };
    let required_body = if required_arms.is_empty() {
        quote!(::std::result::Result::Ok(()))
    } else {
        quote! {
            match partial {
                #(#required_arms,)*
                _ => ::std::result::Result::Ok(()),
            }
        }
    };
    let finish_arms = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        let variant_ident = &variant.ident;
        variant.payload.as_ref().map_or_else(
            || {
                quote! {
                    Partial::#partial_variant => ::std::result::Result::Ok(
                        ::std::option::Option::Some(Self::#variant_ident),
                    )
                }
            },
            |ty| {
                quote! {
                    Partial::#partial_variant(selected) => {
                        ::std::result::Result::Ok(::std::option::Option::Some(
                            Self::#variant_ident(
                                <#ty as #facade::__private::CommandArgs>::finish(selected)?,
                            ),
                        ))
                    }
                }
            },
        )
    });

    quote! {
        #[doc(hidden)]
        const _: () = {
            #keys
            #(#command_tables)*
            static ARGX_SUBCOMMANDS: &[&#facade::__private::Command<'static>] =
                &[#(#command_refs),*];

            #[doc(hidden)]
            pub enum Partial {
                /// No command has been selected yet.
                Unselected,
                #(#partial_variants)*
            }

            impl #impl_generics #facade::__private::Subcommands
                for #ident #ty_generics #where_clause
            {
                type Partial = Partial;

                const COMMANDS: &'static [&'static #facade::__private::Command<'static>] =
                    ARGX_SUBCOMMANDS;

                fn start() -> Self::Partial {
                    Partial::Unselected
                }

                fn selected(partial: &Self::Partial) -> bool {
                    !::std::matches!(partial, Partial::Unselected)
                }

                fn apply(
                    partial: &mut Self::Partial,
                    event: &#facade::__private::Event<'_, '_>,
                ) -> bool {
                    match partial {
                        Partial::Unselected => {}
                        #(#selected_apply_arms)*
                    }
                    let #facade::__private::Event::Command { command } = *event else {
                        return false;
                    };
                    match command.key {
                        #(#select_arms,)*
                        _ => false,
                    }
                }

                fn check_occurrences(
                    #occurrence_partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #occurrence_body
                }

                fn check_required(
                    #required_partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #required_body
                }

                fn finish(
                    partial: Self::Partial,
                ) -> ::std::result::Result<::std::option::Option<Self>, #facade::Error> {
                    match partial {
                        Partial::Unselected => ::std::result::Result::Ok(
                            ::std::option::Option::None,
                        ),
                        #(#finish_arms,)*
                    }
                }
            }
        };
    }
}

/// Adds the payload bounds required by generated subcommand binding.
fn subcommand_generics(subcommand: &model::Subcommand, facade: &TokenStream) -> Generics {
    let mut generics = subcommand.generics.clone();
    let mut bounded = Vec::new();
    for variant in &subcommand.variants {
        let Some(ty) = &variant.payload else {
            continue;
        };
        let rendered = quote!(#ty).to_string();
        if bounded.contains(&rendered) {
            continue;
        }
        bounded.push(rendered);
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #facade::__private::FlattenArgs));
    }
    generics
}
