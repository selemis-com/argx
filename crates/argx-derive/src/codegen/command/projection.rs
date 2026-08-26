//! Privacy-safe semantic type projection for generated commands.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{key, model};

/// Generated semantic type projection for one command declaration.
#[derive(Debug)]
pub(super) struct SemanticProjection {
    /// Helper declarations emitted alongside the derived type.
    pub(super) declarations: TokenStream,
    /// Partial-state type exposed through `CommandArgs`.
    pub(super) partial: TokenStream,
    /// Nominal constructor used when private composed types require an opaque partial witness.
    pub(super) partial_constructor: Option<proc_macro2::Ident>,
    /// Type-level resolver for values owned by this command context.
    pub(super) fields: TokenStream,
    /// Type-level resolver for this command's execution result when directly invocable.
    pub(super) execution: TokenStream,
    /// Type-level resolver for the nested subcommand field, when present.
    pub(super) subcommands: TokenStream,
}

/// Builds privacy-safe semantic value resolvers for this command declaration.
pub(super) fn semantic_projection(
    command: &model::Command,
    facade: &TokenStream,
) -> SemanticProjection {
    let ident = &command.binding.ident;
    let visibility = &command.binding.visibility;
    let suffix = ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    let fields_ident = format_ident!("__ArgxContractFieldsFor{}H{}", suffix, declaration);
    let subcommands_ident = format_ident!("__ArgxContractSubcommandsFor{}H{}", suffix, declaration);
    let partial_ident = partial_type_constructor(command);
    let shape_ident = format_ident!("__ArgxContractShapeFor{}H{}", suffix, declaration);
    let (impl_generics, ty_generics, where_clause) = command.binding.generics.split_for_impl();

    let mut shape_declarations = Vec::new();
    let mut shape_definitions = Vec::new();
    let mut associated_fields = vec![None; command.fields.len()];
    let mut field_bounds = Vec::new();
    let mut field_steps = Vec::new();
    let mut has_fields = false;
    let mut needs_shape = false;

    for (index, field) in command.fields.iter().enumerate() {
        match &field.semantics {
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Flag { .. },
                ..
            }) if field.is_switch() => {
                has_fields = true;
                field_steps.push(quote!(values.flags.push(::std::option::Option::None);));
            }
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Flag { .. },
                ..
            }) => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.value_binding().ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::TypeContractSource
                ));
                field_steps.push(quote! {
                    values.flags.push(::std::option::Option::Some(
                        <<T as #shape_ident>::#associated as
                            #facade::__private::TypeContractSource>::resolve_type(resolver),
                    ));
                });
            }
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Positional,
                ..
            }) => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.value_binding().ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::TypeContractSource
                ));
                field_steps.push(quote! {
                    values.args.push(
                        <<T as #shape_ident>::#associated as
                            #facade::__private::TypeContractSource>::resolve_type(resolver),
                    );
                });
            }
            model::FieldSemantics::Flatten => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.binding.ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::CommandTypeContract
                ));
                field_bounds.push(quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::CommandTypeContract>::Fields:
                        #facade::__private::ResolveValueFields
                ));
                field_steps.push(quote! {
                    <<<T as #shape_ident>::#associated as
                        #facade::__private::CommandTypeContract>::Fields as
                        #facade::__private::ResolveValueFields>::append(resolver, values);
                });
            }
            model::FieldSemantics::Subcommand => {
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.binding.ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated);
            }
        }
    }

    let subcommand_projection = command
        .fields
        .iter()
        .enumerate()
        .find(|(_, field)| matches!(&field.semantics, model::FieldSemantics::Subcommand))
        .map(|(index, _)| {
            associated_fields[index]
                .clone()
                .expect("subcommand fields always have a generated shape association")
        });

    let shape = needs_shape.then(|| {
        quote! {
            trait #shape_ident {
                #(#shape_declarations)*
            }

            impl #impl_generics #shape_ident for #ident #ty_generics #where_clause {
                #(#shape_definitions)*
            }
        }
    });

    // `CommandArgs::Partial` is a public associated type because derives must implement the hidden
    // runtime trait downstream. Keep concrete flattened/subcommand types behind private fields of a
    // nominal witness so a public parser may itself contain private implementation types.
    let mut partial_bounds = Vec::new();
    let partial_types = command
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| match &field.semantics {
            model::FieldSemantics::Flatten => {
                let associated = associated_fields[index]
                    .as_ref()
                    .expect("flatten fields always have a generated shape association");
                partial_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::CommandArgs
                ));
                quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::CommandArgs>::Partial
                )
            }
            model::FieldSemantics::Subcommand => {
                let associated = associated_fields[index]
                    .as_ref()
                    .expect("subcommand fields always have a generated shape association");
                partial_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::Subcommands
                ));
                quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::Subcommands>::Partial
                )
            }
            _ if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) => {
                quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
            }
            _ if field.is_switch() => quote!((bool, bool)),
            _ => quote!((::std::option::Option<#facade::__private::RawValue>, bool)),
        })
        .collect::<Vec<_>>();
    let has_nested_partial = command.fields.iter().any(|field| {
        matches!(
            &field.semantics,
            model::FieldSemantics::Flatten | model::FieldSemantics::Subcommand
        )
    });
    let (partial, partial_constructor, partial_declaration) = if has_nested_partial {
        (
            quote!(#partial_ident<Self>),
            Some(partial_ident.clone()),
            quote! {
                #[doc = "Argx-generated opaque parser partial state."]
                #[doc(hidden)]
                #[allow(
                    missing_copy_implementations,
                    missing_debug_implementations,
                    private_bounds,
                    unreachable_pub,
                    unnameable_types,
                    reason = "generated partial witness hides private composed command types"
                )]
                #visibility struct #partial_ident<T>(#(#partial_types,)*)
                where
                    T: #shape_ident,
                    #(#partial_bounds,)*;
            },
        )
    } else {
        (quote!((#(#partial_types,)*)), None, TokenStream::new())
    };

    let fields = if has_fields {
        quote!(#fields_ident<Self>)
    } else {
        quote!(#facade::__private::NoTypeProjection)
    };
    let field_declaration = has_fields.then(|| {
        let field_where = if needs_shape {
            quote! {
                where
                    T: #shape_ident,
                    #(#field_bounds,)*
            }
        } else {
            TokenStream::new()
        };
        let privacy_allow = needs_shape.then(|| {
            quote! {
                #[allow(
                    private_bounds,
                    reason = "generated contract witnesses intentionally hide concrete value types"
                )]
            }
        });
        quote! {
            #[doc = "Argx-generated semantic contract witness."]
            #[doc(hidden)]
            #[derive(Debug, Clone, Copy)]
            #[allow(
                unreachable_pub,
                unnameable_types,
                reason = "generated witness is exposed only through Argx's hidden projection trait"
            )]
            #visibility struct #fields_ident<T>(::core::marker::PhantomData<fn() -> T>);

            #privacy_allow
            impl<T> #facade::__private::ResolveValueFields for #fields_ident<T>
            #field_where
            {
                fn append(
                    resolver: &mut #facade::__private::TypeResolver,
                    values: &mut #facade::__private::CommandValueTypes,
                ) {
                    #(#field_steps)*
                }
            }
        }
    });

    let directly_invocable = subcommand_projection.is_none();
    let (subcommands, subcommand_declaration) = subcommand_projection.map_or_else(
        || (quote!(#facade::__private::NoTypeProjection), None),
        |associated| {
            let projection = quote!(#subcommands_ident<Self>);
            let declaration = quote! {
                #[doc = "Argx-generated semantic contract witness."]
                #[doc(hidden)]
                #[derive(Debug, Clone, Copy)]
                #[allow(
                    unreachable_pub,
                    unnameable_types,
                    reason = "generated witness is exposed only through Argx's hidden projection trait"
                )]
                #visibility struct #subcommands_ident<T>(::core::marker::PhantomData<fn() -> T>);

                #[allow(
                    private_bounds,
                    reason = "generated contract witnesses intentionally hide concrete value types"
                )]
                impl<T> #facade::__private::ResolveSubcommands for #subcommands_ident<T>
                where
                    T: #shape_ident,
                    <T as #shape_ident>::#associated:
                        #facade::__private::SubcommandTypeContract,
                    <<T as #shape_ident>::#associated as
                        #facade::__private::SubcommandTypeContract>::Commands:
                        #facade::__private::ResolveSubcommandTree,
                {
                    fn resolve(
                        index: usize,
                        rest: &[usize],
                        resolver: &mut #facade::__private::TypeResolver,
                    ) -> ::std::option::Option<#facade::__private::CommandTypes> {
                        <<<T as #shape_ident>::#associated as
                            #facade::__private::SubcommandTypeContract>::Commands as
                            #facade::__private::ResolveSubcommandTree>::resolve(
                                index,
                                rest,
                                resolver,
                            )
                    }
                }
            };
            (projection, Some(declaration))
        },
    );

    let execution = if directly_invocable {
        quote!(Self)
    } else {
        quote!(#facade::__private::NoTypeProjection)
    };

    SemanticProjection {
        declarations: quote! {
            #shape
            #partial_declaration
            #field_declaration
            #subcommand_declaration
        },
        partial,
        partial_constructor,
        fields,
        execution,
        subcommands,
    }
}

/// Returns the deterministic generated partial-state constructor for one command declaration.
fn partial_type_constructor(command: &model::Command) -> proc_macro2::Ident {
    let suffix = command.binding.ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    format_ident!("__ArgxPartialFor{}H{}", suffix, declaration)
}
