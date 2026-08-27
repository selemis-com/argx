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
use crate::{crate_name, key, model};

/// Generated semantic type projection for one subcommand declaration.
#[derive(Debug)]
struct SemanticProjection {
    /// Helper declarations emitted alongside the derived enum.
    declarations: TokenStream,
    /// Type-level resolver for sibling command branches.
    commands: TokenStream,
}

/// Generates static child-command tables and typed enum binding.
pub(crate) fn subcommands(subcommand: &model::Subcommand) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &subcommand.binding.ident;
    let generics = subcommand_generics(subcommand, &facade);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (semantic_impl_generics, semantic_ty_generics, semantic_where_clause) =
        subcommand.binding.generics.split_for_impl();
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
    let semantic_projection = semantic_projection(subcommand, &facade);
    let semantic_declarations = &semantic_projection.declarations;
    let semantic_commands = &semantic_projection.commands;

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
    let env_arms = subcommand
        .variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| {
            let ty = variant.binding.payload.as_ref()?;
            let partial_variant = format_ident!("V{index}");
            Some(quote! {
                Partial::#partial_variant(selected) => {
                    <#ty as #facade::__private::CommandArgs>::apply_env(selected);
                }
            })
        })
        .collect::<Vec<_>>();
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
    let env_partial = if env_arms.is_empty() { quote!(_partial) } else { quote!(partial) };
    let env_body = if env_arms.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            match partial {
                #(#env_arms,)*
                _ => {}
            }
        }
    };
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

    quote! {
        #semantic_declarations

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

            impl #semantic_impl_generics #facade::__private::SubcommandTypeContract
                for #ident #semantic_ty_generics #semantic_where_clause
            {
                type Commands = #semantic_commands;
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
                    #(#select_branches)*
                    false
                }

                fn apply_env(#env_partial: &mut Self::Partial) {
                    #env_body
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

/// Builds privacy-safe semantic branch resolvers for this subcommand declaration.
fn semantic_projection(subcommand: &model::Subcommand, facade: &TokenStream) -> SemanticProjection {
    let ident = &subcommand.binding.ident;
    let visibility = &subcommand.binding.visibility;
    let suffix = ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&subcommand.binding.fingerprint);
    let commands_ident = format_ident!("__ArgxContractCommandsFor{}H{}", suffix, declaration);
    let shape_ident = format_ident!("__ArgxContractShapeFor{}H{}", suffix, declaration);
    let (impl_generics, ty_generics, where_clause) = subcommand.binding.generics.split_for_impl();

    let mut shape_declarations = Vec::new();
    let mut shape_definitions = Vec::new();
    let mut unit_markers = Vec::new();
    let mut bounds = Vec::new();
    let mut arms = Vec::new();

    for (index, variant) in subcommand.variants.iter().enumerate() {
        let associated = format_ident!("Variant{index}");
        shape_declarations.push(quote!(type #associated;));

        if let Some(ty) = &variant.binding.payload {
            shape_definitions.push(quote!(type #associated = #ty;));
            bounds.push(quote!(
                <T as #shape_ident>::#associated: #facade::ContractCommand
            ));
            arms.push(quote! {
                #index => <<T as #shape_ident>::#associated as
                    #facade::__private::ResolveCommandTypeContract>::contract_types(
                        rest,
                        resolver,
                    ),
            });
        } else {
            let variant_ident = &variant.binding.ident;
            let marker = format_ident!(
                "{}{}NeedsArgsPayloadForContractDiscoveryH{}",
                suffix,
                variant_ident,
                declaration,
            );
            unit_markers.push(quote! {
                #[doc = "Argx marker for a unit subcommand that cannot carry an execution identity."]
                #[doc(hidden)]
                #[derive(Debug, Clone, Copy)]
                struct #marker;
            });
            shape_definitions.push(quote!(type #associated = #marker;));
            bounds.push(quote!(
                <T as #shape_ident>::#associated:
                    #facade::UnitSubcommandContractRequiresArgsPayload
            ));
            arms.push(quote!(#index => ::std::option::Option::None,));
        }
    }

    SemanticProjection {
        declarations: quote! {
            #(#unit_markers)*

            trait #shape_ident {
                #(#shape_declarations)*
            }

            impl #impl_generics #shape_ident for #ident #ty_generics #where_clause {
                #(#shape_definitions)*
            }

            #[doc = "Argx-generated semantic contract witness."]
            #[doc(hidden)]
            #[derive(Debug, Clone, Copy)]
            #[allow(
                unreachable_pub,
                unnameable_types,
                reason = "generated witness is exposed only through Argx's hidden projection trait"
            )]
            #visibility struct #commands_ident<T>(::core::marker::PhantomData<fn() -> T>);

            #[allow(
                private_bounds,
                reason = "generated contract witnesses intentionally hide concrete payload types"
            )]
            impl<T> #facade::__private::ResolveSubcommandTree for #commands_ident<T>
            where
                T: #shape_ident,
                #(#bounds,)*
            {
                fn resolve(
                    index: usize,
                    rest: &[usize],
                    resolver: &mut #facade::__private::TypeResolver,
                ) -> ::std::option::Option<#facade::__private::CommandTypes> {
                    match index {
                        #(#arms)*
                        _ => ::std::option::Option::None,
                    }
                }
            }
        },
        commands: quote!(#commands_ident<Self>),
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
