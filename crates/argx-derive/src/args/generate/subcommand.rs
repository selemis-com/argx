//! Code generation for `Subcommand` enums.
//!
//! Subcommand generation keeps sibling selection separate from command payload binding. Exactly one
//! variant can become active, so generated partial state is an enum rather than a tuple containing
//! every sibling. Once selected, events are delegated only to that branch, with unclaimed events
//! left available to containing commands for inherited global options.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Generics, parse_quote};

use super::option_str;
use crate::{
    args::{key, model},
    support,
};

/// Generates static child-command tables and typed enum binding.
pub(crate) fn subcommands(subcommand: &model::Subcommand) -> TokenStream {
    let facade = support::facade_path();
    let ident = &subcommand.binding.ident;
    let generics = subcommand_generics(subcommand, &facade);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let keys = key::subcommand_constants(
        &facade,
        &subcommand.binding.fingerprint,
        subcommand.variants.len(),
    );

    // Each variant gets one command projection. A payload command is composed beneath the variant
    // metadata without creating an extra visible command segment.
    let command_tables = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let table = format_ident!("ARGX_SUBCOMMAND_{index}");
        let version_table = format_ident!("ARGX_VERSION_ACTION_{index}");
        let key = key::ident("SUBCOMMAND", Some(index));
        let name = &variant.semantics.name;
        let about = option_str(variant.semantics.about.as_deref());
        let own_description = variant.semantics.description.as_deref();
        let own_help_sections = variant
            .semantics
            .help_sections
            .iter()
            .map(|section| {
                let heading = &section.heading;
                let body = &section.body;
                quote! {
                    #facade::__private::HelpSection {
                        heading: #heading,
                        body: #body,
                    }
                }
            })
            .collect::<Vec<_>>();
        let aliases = &variant.semantics.aliases;
        let short_version =
            variant.semantics.version.as_ref().or(variant.semantics.long_version.as_ref());
        let long_version =
            variant.semantics.long_version.as_ref().or(variant.semantics.version.as_ref());
        let version_action = short_version.zip(long_version).map(|(short, long)| {
            quote! {
                static #version_table: #facade::__private::Action<'static> =
                    #facade::__private::Action {
                        name: "version",
                        diagnostic: "--version",
                        help: "Print version",
                        longs: &["version"],
                        shorts: b"V",
                        kind: #facade::__private::ActionKind::Version {
                            short: #short,
                            long: #long,
                        },
                    };
            }
        });
        let actions = if version_action.is_some() {
            quote!(&[&#facade::__private::HELP_ACTION, &#version_table])
        } else {
            quote!(&[&#facade::__private::HELP_ACTION])
        };
        let command = variant.binding.payload.as_ref().map_or_else(|| {
            let description = option_str(own_description);
            quote! {
                static #table: #facade::__private::Command<'static> =
                    #facade::__private::Command {
                        name: #name,
                        about: #about,
                        description: #description,
                        help_sections: &[#(#own_help_sections),*],
                        help_groups: &[],
                        aliases: &[#(#aliases),*],
                        actions: #actions,
                        flags: &[],
                        args: &[],
                        constraints: &[],
                        subcommands: &[],
                        key: #key,
                    };
            }
        }, |ty| {
            let description = own_description.map_or_else(
                || quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.description),
                |description| quote!(::std::option::Option::Some(#description)),
            );
            let help_sections = if variant.semantics.help_sections.is_empty() {
                quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.help_sections)
            } else {
                quote!(&[#(#own_help_sections),*])
            };
            quote! {
                static #table: #facade::__private::Command<'static> =
                    #facade::__private::Command {
                        name: #name,
                        about: #about,
                        description: #description,
                        help_sections: #help_sections,
                        help_groups: <#ty as #facade::__private::CommandArgs>::COMMAND.help_groups,
                        aliases: &[#(#aliases),*],
                        actions: #actions,
                        flags: <#ty as #facade::__private::CommandArgs>::COMMAND.flags,
                        args: <#ty as #facade::__private::CommandArgs>::COMMAND.args,
                        constraints: <#ty as #facade::__private::CommandArgs>::COMMAND.constraints,
                        subcommands: <#ty as #facade::__private::CommandArgs>::COMMAND.subcommands,
                        key: #key,
                    };
            }
        });
        quote! {
            #version_action
            #command
            const _: () = ::core::assert!(
                #facade::__private::action_flag_spellings_disjoint(#table.actions, #table.flags),
                "subcommand contains a flag spelling reserved by a built-in action",
            );
        }
    });
    let command_refs = (0..subcommand.variants.len()).map(|index| {
        let table = format_ident!("ARGX_SUBCOMMAND_{index}");
        quote!(&#table)
    });

    // Only one sibling command can be active. An enum keeps the accumulator proportional to the
    // largest selected branch instead of reserving space for every sibling's partial state.
    let partial_variants = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        variant.binding.payload.as_ref().map_or_else(
            || quote!(#partial_variant,),
            |ty| quote!(#partial_variant(<#ty as #facade::__private::CommandArgs>::Partial),),
        )
    });

    // Once a sibling is selected, only that branch may consume descendant events. Returning false
    // for unclaimed events is what lets a containing command bind inherited global options.
    let selected_apply_arms = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        variant.binding.payload.as_ref().map_or_else(
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
    // Selection is driven by semantic command keys emitted into raw `Command` events, not by
    // re-matching user-facing names in generated typed binding.
    let select_branches = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let table = format_ident!("ARGX_SUBCOMMAND_{index}");
        let partial_variant = format_ident!("V{index}");
        variant.binding.payload.as_ref().map_or_else(
            || {
                quote! {
                    if ::std::ptr::eq(command, &#table) {
                        *partial = Partial::#partial_variant;
                        return true;
                    }
                }
            },
            |ty| {
                quote! {
                    if ::std::ptr::eq(command, &#table) {
                        *partial = Partial::#partial_variant(
                            <#ty as #facade::__private::CommandArgs>::start(),
                        );
                        return true;
                    }
                }
            },
        )
    });

    // Every post-parse validation stage delegates only into the selected payload branch. Unit
    // variants need no generated arm because they have no nested binding state.
    let occurrence_arms = subcommand
        .variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| {
            let ty = variant.binding.payload.as_ref()?;
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
            let ty = variant.binding.payload.as_ref()?;
            let partial_variant = format_ident!("V{index}");
            Some(quote! {
                Partial::#partial_variant(selected) => {
                    <#ty as #facade::__private::CommandArgs>::check_required(selected)
                }
            })
        })
        .collect::<Vec<_>>();
    let constraint_arms = subcommand
        .variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| {
            let ty = variant.binding.payload.as_ref()?;
            let partial_variant = format_ident!("V{index}");
            Some(quote! {
                Partial::#partial_variant(selected) => {
                    <#ty as #facade::__private::CommandArgs>::check_constraints(selected)
                }
            })
        })
        .collect::<Vec<_>>();
    let occurrence_partial =
        if occurrence_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
    let required_partial =
        if required_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
    let constraint_partial =
        if constraint_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
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
    let constraint_body = if constraint_arms.is_empty() {
        quote!(::std::result::Result::Ok(()))
    } else {
        quote! {
            match partial {
                #(#constraint_arms,)*
                _ => ::std::result::Result::Ok(()),
            }
        }
    };
    let finish_arms = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let partial_variant = format_ident!("V{index}");
        let variant_ident = &variant.binding.ident;
        variant.binding.payload.as_ref().map_or_else(
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

    let schema_impl = subcommand.binding.schema.then(|| {
        let registrations = subcommand.variants.iter().enumerate().map(|(index, variant)| {
            let ty = variant.binding.payload.as_ref().expect("unit variants were rejected above");
            quote! {
                <#ty as #facade::__private::SchemaCommand>::register_schema_commands(
                    commands[#index],
                    registry,
                );
            }
        });
        let count = subcommand.variants.len();
        quote! {
            impl #impl_generics #facade::__private::SchemaSubcommands
                for #ident #ty_generics #where_clause
            {
                fn register_schema_subcommands(
                    commands: &'static [&'static #facade::__private::Command<'static>],
                    registry: &mut #facade::__private::SchemaRegistry,
                ) {
                    assert_eq!(
                        commands.len(),
                        #count,
                        "generated schema topology diverged from subcommand metadata",
                    );
                    #(#registrations)*
                }
            }
        }
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

            #schema_impl

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
                    #(#select_branches)*
                    false
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

                fn check_constraints(
                    #constraint_partial: &Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #constraint_body
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
    let mut generics = subcommand.binding.generics.clone();
    let mut bounded = Vec::new();
    for variant in &subcommand.variants {
        let Some(ty) = &variant.binding.payload else {
            continue;
        };
        let rendered = quote!(#ty).to_string();
        if bounded.contains(&rendered) {
            continue;
        }
        bounded.push(rendered);
        generics.make_where_clause().predicates.push(parse_quote!(#ty: #facade::Args));
    }
    generics
}
