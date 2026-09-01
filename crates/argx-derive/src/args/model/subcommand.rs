//! Normalization and validation for `Subcommand` declarations.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields};

use super::{
    CommandSemantics, GenericName, GenericUse, Subcommand, SubcommandBinding, Variant,
    VariantBinding,
    shape::{peel_option, peel_vec},
};
use crate::{args::attrs, support};

impl Subcommand {
    /// Parses and validates a subcommand enum before code generation.
    pub(crate) fn from_input(input: &DeriveInput) -> syn::Result<Self> {
        let Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Subcommand can only be derived for enums",
            ));
        };
        let attributes = attrs::command(&input.attrs)?;
        if attributes.name.is_some()
            || attributes.about.is_some()
            || attributes.version.is_some()
            || attributes.long_version.is_some()
            || !attributes.aliases.is_empty()
            || !attributes.one_of.is_empty()
            || !attributes.any_of.is_empty()
        {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Subcommand enum attributes support only `schema`",
            ));
        }
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
            let mut attributes = attrs::variant(&variant.attrs)?;
            if attributes.schema {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`schema` is only valid on Parser, Args, or Subcommand declarations",
                ));
            }
            if !attributes.one_of.is_empty() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`one_of` is only valid on Parser or Args declarations",
                ));
            }
            if !attributes.any_of.is_empty() {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "`any_of` is only valid on Parser or Args declarations",
                ));
            }
            let rust_name = support::ident_name(&variant.ident);
            let name = attributes.name.take().unwrap_or_else(|| support::to_kebab(&rust_name));
            let semantics =
                CommandSemantics::from_attrs(name, attributes, attrs::doc_help(&variant.attrs));
            validate_subcommand_name(&semantics.name, variant.ident.span())?;
            if spellings.contains(&semantics.name) {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    format!("duplicate subcommand `{}`", semantics.name),
                ));
            }
            spellings.push(semantics.name.clone());

            for alias in &semantics.aliases {
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
                semantics,
            });
        }

        if attributes.schema {
            for variant in &variants {
                if variant.binding.payload.is_none() {
                    return Err(syn::Error::new_spanned(
                        &variant.binding.ident,
                        "`#[argx(schema)]` requires executable subcommands to use a concrete Args payload; use an empty Args struct instead of a unit variant",
                    ));
                }
            }
        }
        validate_variant_generics(&variants, &input.generics)?;

        Ok(Self {
            binding: SubcommandBinding {
                ident: input.ident.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
                schema: attributes.schema,
            },
            variants,
        })
    }
}

/// Rejects subcommand payloads that depend on the enum's generic parameters.
fn validate_variant_generics(variants: &[Variant], generics: &syn::Generics) -> syn::Result<()> {
    let params = GenericName::collect(generics);
    if params.is_empty() {
        return Ok(());
    }

    for variant in variants {
        let Some(ty) = &variant.binding.payload else {
            continue;
        };
        if GenericUse::finds(&params, ty) {
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
    if name == "schema" {
        return Err(syn::Error::new(span, "`schema` is reserved by Argx"));
    }
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
