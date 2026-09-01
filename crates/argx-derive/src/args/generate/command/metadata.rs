//! Static command-table generation and composition.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::args::{key, model};

/// Generated static table expressions for one command declaration.
#[derive(Debug)]
pub(super) struct Tables {
    /// Declarations required to compose flattened child tables.
    pub(super) decls: TokenStream,
    /// Final flag slice expression stored on the command.
    pub(super) flags: TokenStream,
    /// Final positional slice expression stored on the command.
    pub(super) args: TokenStream,
    /// Final normalized constraint slice expression stored on the command.
    pub(super) constraints: TokenStream,
    /// Final flattened help-group slice expression stored on the command.
    pub(super) help_groups: TokenStream,
}

/// Generates normalized relationship tables for arguments declared directly on this command.
pub(super) fn constraint_tables(
    command: &model::Command,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> Vec<TokenStream> {
    let mut generated = Vec::new();
    let mut flag_index = 0_usize;
    let mut arg_index = 0_usize;
    let mut constraint_index = 0_usize;

    for field in &command.fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        let source = match &argument.kind {
            model::ArgumentKind::Flag { .. } => {
                let key = key::ident("FLAG", Some(flag_index));
                flag_index += 1;
                key
            }
            model::ArgumentKind::Positional => {
                let key = key::ident("ARG", Some(arg_index));
                arg_index += 1;
                key
            }
        };

        for (kind, target) in argument
            .requires
            .iter()
            .map(|target| (quote!(#facade::__private::ConstraintKind::Requires), target))
            .chain(
                argument
                    .conflicts
                    .iter()
                    .map(|target| (quote!(#facade::__private::ConstraintKind::Conflicts), target)),
            )
        {
            let table = format_ident!("ARGX_CONSTRAINT_{constraint_index}");
            constraint_index += 1;
            let target = constraint_target(command, target, facade, flags, args);
            generated.push(quote! {
                const #table: #facade::__private::Constraint = #facade::__private::Constraint {
                    kind: #kind,
                    source: #source,
                    target: #target,
                };
            });
        }
    }

    generated
}

/// Resolves a relationship target to a direct semantic key or a composed flattened lookup.
fn constraint_target(
    command: &model::Command,
    target: &str,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> TokenStream {
    let mut flag_index = 0_usize;
    let mut arg_index = 0_usize;
    for field in &command.fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        let resolved = match &argument.kind {
            model::ArgumentKind::Flag { .. } => {
                let resolved = key::ident("FLAG", Some(flag_index));
                flag_index += 1;
                resolved
            }
            model::ArgumentKind::Positional => {
                let resolved = key::ident("ARG", Some(arg_index));
                arg_index += 1;
                resolved
            }
        };
        if field.binding.name == target {
            return quote!(#resolved);
        }
    }

    quote!(#facade::__private::argument_key_by_name(#flags, #args, #target))
}

/// Generates and composes command-level `one_of` groups.
pub(super) fn one_of_tables(
    command: &model::Command,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> (TokenStream, TokenStream) {
    let own_members = command.semantics.one_of.iter().enumerate().map(|(index, group)| {
        let members = format_ident!("ARGX_ONE_OF_MEMBERS_{index}");
        let keys = group
            .iter()
            .map(|target| quote!(#facade::__private::argument_key_by_name(#flags, #args, #target)));
        let len = group.len();
        quote! {
            static #members: [#facade::__private::Key; #len] = [#(#keys),*];
        }
    });
    let own_entries = command.semantics.one_of.iter().enumerate().map(|(index, _)| {
        let members = format_ident!("ARGX_ONE_OF_MEMBERS_{index}");
        quote!(#facade::__private::OneOf { members: &#members })
    });
    let own_len = command.semantics.one_of.len();

    let flattened = command.fields.iter().filter_map(|field| {
        if !field.is_flatten() {
            return None;
        }
        let ty = &field.binding.ty;
        Some(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.one_of))
    });

    let declarations = quote! {
        #(#own_members)*
        static ARGX_OWN_ONE_OF: [#facade::__private::OneOf<'static>; #own_len] =
            [#(#own_entries),*];
        const ARGX_ONE_OF_GROUPS: &[&[#facade::__private::OneOf<'static>]] =
            &[#(#flattened,)* &ARGX_OWN_ONE_OF];
        static ARGX_ONE_OF: [#facade::__private::OneOf<'static>;
            #facade::__private::table_len(ARGX_ONE_OF_GROUPS)] =
            #facade::__private::concat_one_of(ARGX_ONE_OF_GROUPS);
    };

    (declarations, quote!(&ARGX_ONE_OF))
}

/// Generates and composes command-level `any_of` groups.
pub(super) fn any_of_tables(
    command: &model::Command,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> (TokenStream, TokenStream) {
    let own_members = command.semantics.any_of.iter().enumerate().map(|(index, group)| {
        let members = format_ident!("ARGX_ANY_OF_MEMBERS_{index}");
        let keys = group
            .iter()
            .map(|target| quote!(#facade::__private::argument_key_by_name(#flags, #args, #target)));
        let len = group.len();
        quote! {
            static #members: [#facade::__private::Key; #len] = [#(#keys),*];
        }
    });
    let own_entries = command.semantics.any_of.iter().enumerate().map(|(index, _)| {
        let members = format_ident!("ARGX_ANY_OF_MEMBERS_{index}");
        quote!(#facade::__private::AnyOf { members: &#members })
    });
    let own_len = command.semantics.any_of.len();

    let flattened = command.fields.iter().filter_map(|field| {
        if !field.is_flatten() {
            return None;
        }
        let ty = &field.binding.ty;
        Some(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.any_of))
    });

    let declarations = quote! {
        #(#own_members)*
        static ARGX_OWN_ANY_OF: [#facade::__private::AnyOf<'static>; #own_len] =
            [#(#own_entries),*];
        const ARGX_ANY_OF_GROUPS: &[&[#facade::__private::AnyOf<'static>]] =
            &[#(#flattened,)* &ARGX_OWN_ANY_OF];
        static ARGX_ANY_OF: [#facade::__private::AnyOf<'static>;
            #facade::__private::table_len(ARGX_ANY_OF_GROUPS)] =
            #facade::__private::concat_any_of(ARGX_ANY_OF_GROUPS);
    };

    (declarations, quote!(&ARGX_ANY_OF))
}

/// Builds one flat parse table from this declaration's own fields and flattened children.
pub(super) fn command_tables(
    command: &model::Command,
    facade: &TokenStream,
    flag_count: usize,
    arg_count: usize,
    constraint_count: usize,
) -> Tables {
    if !command.fields.iter().any(model::Field::is_flatten) {
        let flags = (0..flag_count).map(|index| {
            let table = format_ident!("ARGX_FLAG_{index}");
            quote!(&#table)
        });
        let args = (0..arg_count).map(|index| {
            let table = format_ident!("ARGX_ARG_{index}");
            quote!(&#table)
        });
        let constraints = (0..constraint_count).map(|index| {
            let table = format_ident!("ARGX_CONSTRAINT_{index}");
            quote!(#table)
        });
        return Tables {
            decls: TokenStream::new(),
            flags: quote!(&[#(#flags),*]),
            args: quote!(&[#(#args),*]),
            constraints: quote!(&[#(#constraints),*]),
            help_groups: quote!(&[]),
        };
    }

    let mut flag_groups = Vec::new();
    let mut arg_groups = Vec::new();
    let mut constraint_groups = Vec::new();
    let mut help_group_groups = Vec::new();
    let mut help_group_decls = Vec::new();
    let mut own_flags = Vec::new();
    let mut own_args = Vec::new();
    let mut own_constraints = Vec::new();
    let mut flag_at = 0_usize;
    let mut arg_at = 0_usize;
    let mut constraint_at = 0_usize;
    let mut flatten_checks = Vec::new();

    fn flush_flags(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|index| {
            let table = format_ident!("ARGX_FLAG_{index}");
            quote!(&#table)
        });
        groups.push(quote!(&[#(#refs),*]));
        own.clear();
    }

    fn flush_args(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|index| {
            let table = format_ident!("ARGX_ARG_{index}");
            quote!(&#table)
        });
        groups.push(quote!(&[#(#refs),*]));
        own.clear();
    }

    fn flush_constraints(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let entries = own.iter().map(|index| {
            let table = format_ident!("ARGX_CONSTRAINT_{index}");
            quote!(#table)
        });
        groups.push(quote!(&[#(#entries),*]));
        own.clear();
    }

    for field in &command.fields {
        match &field.semantics {
            model::FieldSemantics::Argument(
                argument @ model::Argument { kind: model::ArgumentKind::Flag { .. }, .. },
            ) => {
                own_flags.push(flag_at);
                flag_at += 1;
                for _ in argument.requires.iter().chain(&argument.conflicts) {
                    own_constraints.push(constraint_at);
                    constraint_at += 1;
                }
            }
            model::FieldSemantics::Argument(
                argument @ model::Argument { kind: model::ArgumentKind::Positional, .. },
            ) => {
                own_args.push(arg_at);
                arg_at += 1;
                for _ in argument.requires.iter().chain(&argument.conflicts) {
                    own_constraints.push(constraint_at);
                    constraint_at += 1;
                }
            }
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                flush_flags(&mut own_flags, &mut flag_groups);
                flush_args(&mut own_args, &mut arg_groups);
                flush_constraints(&mut own_constraints, &mut constraint_groups);
                flag_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.flags));
                arg_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.args));
                constraint_groups
                    .push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.constraints));
                if let Some(heading) = field.help_heading.as_deref() {
                    let group = format_ident!("ARGX_HELP_GROUP_{}", help_group_decls.len());
                    help_group_decls.push(quote! {
                        static #group: #facade::__private::HelpGroup<'static> =
                            #facade::__private::HelpGroup {
                                heading: #heading,
                                flags: <#ty as #facade::__private::CommandArgs>::COMMAND.flags,
                                args: <#ty as #facade::__private::CommandArgs>::COMMAND.args,
                            };
                    });
                    help_group_groups.push(quote!(&[&#group]));
                } else {
                    help_group_groups.push(
                        quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.help_groups),
                    );
                }
                flatten_checks.push(quote! {
                    const _: () = ::core::assert!(
                        <#ty as #facade::__private::CommandArgs>::COMMAND.subcommands.is_empty(),
                        "flattened Args cannot declare subcommands",
                    );
                });
            }
            model::FieldSemantics::Subcommand => {}
        }
    }
    flush_flags(&mut own_flags, &mut flag_groups);
    flush_args(&mut own_args, &mut arg_groups);
    flush_constraints(&mut own_constraints, &mut constraint_groups);

    debug_assert_eq!(flag_at, flag_count);
    debug_assert_eq!(arg_at, arg_count);
    debug_assert_eq!(constraint_at, constraint_count);

    Tables {
        decls: quote! {
            #(#flatten_checks)*
            #(#help_group_decls)*
            const ARGX_FLAG_GROUPS: &[&[&#facade::__private::Flag<'static>]] =
                &[#(#flag_groups),*];
            const ARGX_ARG_GROUPS: &[&[&#facade::__private::Arg<'static>]] =
                &[#(#arg_groups),*];
            static ARGX_FLAGS: [&#facade::__private::Flag<'static>;
                #facade::__private::table_len(ARGX_FLAG_GROUPS)] =
                #facade::__private::concat_flags(ARGX_FLAG_GROUPS);
            static ARGX_ARGS: [&#facade::__private::Arg<'static>;
                #facade::__private::table_len(ARGX_ARG_GROUPS)] =
                #facade::__private::concat_args(ARGX_ARG_GROUPS);
            const ARGX_HELP_GROUP_GROUPS: &[&[&#facade::__private::HelpGroup<'static>]] =
                &[#(#help_group_groups),*];
            static ARGX_HELP_GROUPS: [&#facade::__private::HelpGroup<'static>;
                #facade::__private::table_len(ARGX_HELP_GROUP_GROUPS)] =
                #facade::__private::concat_help_groups(ARGX_HELP_GROUP_GROUPS);
            const ARGX_CONSTRAINT_GROUPS: &[&[#facade::__private::Constraint]] =
                &[#(#constraint_groups),*];
            static ARGX_CONSTRAINTS: [#facade::__private::Constraint;
                #facade::__private::table_len(ARGX_CONSTRAINT_GROUPS)] =
                #facade::__private::concat_constraints(ARGX_CONSTRAINT_GROUPS);
        },
        flags: quote!(&ARGX_FLAGS),
        args: quote!(&ARGX_ARGS),
        constraints: quote!(&ARGX_CONSTRAINTS),
        help_groups: quote!(&ARGX_HELP_GROUPS),
    }
}
