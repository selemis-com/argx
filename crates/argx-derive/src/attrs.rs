//! Parsing for Argx container and field attributes.

use syn::{Attribute, LitChar, LitStr, Token};

/// An attribute whose bare form asks Argx to infer a value from the Rust identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inferred<T> {
    /// The attribute was present without an explicit value.
    Infer,
    /// The attribute supplied an explicit value.
    Explicit(T),
}

/// Attributes accepted on a command struct.
#[derive(Debug, Default)]
pub(crate) struct CommandAttrs {
    /// Explicit command-line name.
    pub name: Option<String>,
}

/// Attributes accepted on a command field.
#[derive(Debug, Default)]
pub(crate) struct FieldAttrs {
    /// Whether this field contributes another `Args` declaration inline.
    pub flatten: bool,
    /// Long flag spelling, or an instruction to infer it.
    pub long: Option<Inferred<String>>,
    /// Short flag spelling, or an instruction to infer it.
    pub short: Option<Inferred<char>>,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
}

/// Parses every `#[argx(...)]` attribute on a command declaration.
pub(crate) fn command(attributes: &[Attribute]) -> syn::Result<CommandAttrs> {
    let mut parsed = CommandAttrs::default();
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("argx")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if parsed.name.is_some() {
                    return Err(meta.error("duplicate `name` attribute"));
                }
                let value = meta.value()?.parse::<LitStr>()?;
                parsed.name = Some(value.value());
                Ok(())
            } else {
                Err(meta.error("unsupported Argx command attribute"))
            }
        })?;
    }
    Ok(parsed)
}

/// Parses every `#[argx(...)]` attribute on a field declaration.
pub(crate) fn field(attributes: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut parsed = FieldAttrs::default();
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("argx")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("flatten") {
                if parsed.flatten {
                    return Err(meta.error("duplicate `flatten` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`flatten` takes no value"));
                }
                parsed.flatten = true;
                Ok(())
            } else if meta.path.is_ident("long") {
                if parsed.long.is_some() {
                    return Err(meta.error("duplicate `long` attribute"));
                }
                parsed.long = Some(if meta.input.peek(Token![=]) {
                    Inferred::Explicit(meta.value()?.parse::<LitStr>()?.value())
                } else {
                    Inferred::Infer
                });
                Ok(())
            } else if meta.path.is_ident("short") {
                if parsed.short.is_some() {
                    return Err(meta.error("duplicate `short` attribute"));
                }
                parsed.short = Some(if meta.input.peek(Token![=]) {
                    Inferred::Explicit(meta.value()?.parse::<LitChar>()?.value())
                } else {
                    Inferred::Infer
                });
                Ok(())
            } else if meta.path.is_ident("allow_hyphen_values") {
                if parsed.allow_hyphen_values {
                    return Err(meta.error("duplicate `allow_hyphen_values` attribute"));
                }
                parsed.allow_hyphen_values = true;
                Ok(())
            } else if meta.path.is_ident("allow_negative_numbers") {
                if parsed.allow_negative_numbers {
                    return Err(meta.error("duplicate `allow_negative_numbers` attribute"));
                }
                parsed.allow_negative_numbers = true;
                Ok(())
            } else {
                Err(meta.error("unsupported Argx field attribute"))
            }
        })?;
    }
    Ok(parsed)
}

/// Rejects Argx attributes on a declaration whose attribute vocabulary is not implemented yet.
pub(crate) fn reject(attributes: &[Attribute], context: &str) -> syn::Result<()> {
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("argx")) {
        attribute.parse_nested_meta(|meta| {
            Err(meta.error(format!("unsupported Argx {context} attribute")))
        })?;
    }
    Ok(())
}
