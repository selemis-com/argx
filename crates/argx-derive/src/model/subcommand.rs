//! Normalization and validation for `Subcommand` declarations.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields, GenericParam, visit::Visit as _};

use super::{
    CommandSemantics, GenericName, GenericUse, HelpSection, Subcommand, SubcommandBinding, Variant,
    VariantBinding, ident_name,
    shape::{peel_option, peel_vec},
};
use crate::{attrs, case};

impl Subcommand {
    /// Parses and validates a subcommand enum before code generation.
    pub(crate) fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Subcommand can only be derived for enums",
            ));
        };
        attrs::reject(&input.attrs, "subcommand")?;
        if data.variants.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Subcommand requires at least one variant",
            ));
        }

        // Canonical names and aliases share one sibling namespace. Keeping a single spelling set
        // here prevents lookup order from deciding an otherwise ambiguous subcommand.
        let mut spellings = Vec::<String>::new();
        let mut variants = Vec::with_capacity(data.variants.len());
        for variant in &data.variants {
            let attributes = attrs::variant(&variant.attrs)?;
            let rust_name = ident_name(&variant.ident);
            let name = attributes.name.unwrap_or_else(|| case::to_kebab(&rust_name));
            let docs = attrs::doc_help(&variant.attrs);
            let about = attributes.about.clone().or(docs.summary);
            let description = attributes.about.or(docs.description);
            let help_sections = docs
                .sections
                .into_iter()
                .map(|section| HelpSection { heading: section.heading, body: section.body })
                .collect();
            let version = attributes.version;
            let long_version = attributes.long_version;
            validate_subcommand_name(&name, variant.ident.span())?;
            if spellings.contains(&name) {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    format!("duplicate subcommand `{name}`"),
                ));
            }
            spellings.push(name.clone());

            let aliases = attributes.aliases;
            for alias in &aliases {
                validate_subcommand_name(alias, variant.ident.span())?;
                if spellings.contains(alias) {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        format!("duplicate subcommand spelling `{alias}`"),
                    ));
                }
                spellings.push(alias.clone());
            }

            let payload = match &variant.fields {
                Fields::Unit => None,
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field = &fields.unnamed[0];
                    attrs::reject(&field.attrs, "subcommand payload")?;
                    if peel_option(&field.ty).is_some() || peel_vec(&field.ty).is_some() {
                        return Err(syn::Error::new_spanned(
                            &field.ty,
                            "subcommand payload must be one direct Args type",
                        ));
                    }
                    Some(field.ty.clone())
                }
                Fields::Unnamed(_) => {
                    return Err(syn::Error::new_spanned(
                        &variant.fields,
                        "subcommand tuple variants must contain exactly one Args payload",
                    ));
                }
                Fields::Named(_) => {
                    return Err(syn::Error::new_spanned(
                        &variant.fields,
                        "subcommand variants support only unit variants or one unnamed Args payload",
                    ));
                }
            };

            variants.push(Variant {
                binding: VariantBinding { ident: variant.ident.clone(), payload },
                semantics: CommandSemantics {
                    name,
                    about,
                    description,
                    help_sections,
                    version,
                    long_version,
                    aliases,
                },
            });
        }

        validate_variant_generics(&variants, &input.generics)?;

        Ok(Self {
            binding: SubcommandBinding {
                ident: input.ident.clone(),
                visibility: input.vis.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
            },
            variants,
        })
    }
}

/// Rejects subcommand payloads that depend on the enum's generic parameters.
fn validate_variant_generics(variants: &[Variant], generics: &syn::Generics) -> syn::Result<()> {
    let params = generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Type(param) => GenericName::Ident(param.ident.clone()),
            GenericParam::Const(param) => GenericName::Ident(param.ident.clone()),
            GenericParam::Lifetime(param) => GenericName::Lifetime(param.lifetime.ident.clone()),
        })
        .collect::<Vec<_>>();
    if params.is_empty() {
        return Ok(());
    }

    for variant in variants {
        let Some(ty) = &variant.binding.payload else {
            continue;
        };
        let mut visitor = GenericUse { params: &params, found: false };
        visitor.visit_type(ty);
        if visitor.found {
            return Err(syn::Error::new_spanned(
                ty,
                "subcommand payload cannot depend on the enum's generic parameters; use a concrete Args type",
            ));
        }
    }
    Ok(())
}

/// Validates one command-line spelling used to select a subcommand.
fn validate_subcommand_name(name: &str, span: Span) -> syn::Result<()> {
    if name.is_empty()
        || name.starts_with('-')
        || name.contains('=')
        || name.chars().any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(syn::Error::new(
            span,
            "subcommand name must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls",
        ));
    }
    Ok(())
}
