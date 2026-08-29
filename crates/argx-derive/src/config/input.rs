//! Parsing and validation of unified Argx configuration declarations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Error as SynError, Expr, Field as SynField, Fields,
    GenericArgument, Ident, LitChar, LitStr, PathArguments, Result as SynResult, Token, Type,
    Visibility,
};

/// Canonical derive-time model for a unified configuration struct.
pub(crate) struct Config {
    /// Rust identifier of the configuration type.
    pub(crate) ident: Ident,
    /// Visibility inherited by generated helper types.
    pub(crate) visibility: Visibility,
    /// Optional environment-variable prefix declared on the configuration.
    pub(crate) prefix: Option<LitStr>,
    /// Parsed configuration fields in declaration order.
    pub(crate) fields: Vec<Field>,
}

impl Config {
    /// Parses and validates a `Config` derive input.
    pub(crate) fn parse(input: &DeriveInput) -> SynResult<Self> {
        let prefix = parse_config_attributes(&input.attrs)?;
        if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
            return Err(SynError::new_spanned(
                &input.generics,
                "Config does not support generic configuration types",
            ));
        }
        let Data::Struct(data) = &input.data else {
            return Err(SynError::new_spanned(
                &input.ident,
                "Config can only be derived for structs",
            ));
        };
        let Fields::Named(fields) = &data.fields else {
            return Err(SynError::new_spanned(
                &data.fields,
                "Config requires a struct with named fields",
            ));
        };
        let fields = fields.named.iter().map(Field::parse).collect::<SynResult<Vec<_>>>()?;
        Ok(Self { ident: input.ident.clone(), visibility: input.vis.clone(), prefix, fields })
    }
}

/// Canonical derive-time model for one configuration field.
pub(crate) struct Field {
    /// Rust field identifier.
    pub(crate) ident: Ident,
    /// Unraw field name used by configuration sources.
    pub(crate) name: String,
    /// Declared Rust field type.
    pub(crate) ty: Type,
    /// Whether the field is an `Option<T>`.
    pub(crate) optional: bool,
    /// Whether the field is a `Vec<T>`.
    pub(crate) many: bool,
    /// Optional declared fallback value.
    pub(crate) default: Option<DefaultValue>,
    /// Optional exact environment-variable mapping.
    pub(crate) env: Option<LitStr>,
    /// Whether this field contains a flattened nested configuration.
    pub(crate) nested: bool,
    /// Whether collection values accept comma-delimited scalar input.
    pub(crate) delimited: bool,
    /// CLI-only metadata copied to the generated sparse argv adapter.
    pub(crate) cli: Vec<TokenStream>,
    /// Documentation attributes copied to generated CLI fields.
    pub(crate) docs: Vec<Attribute>,
}

impl Field {
    /// Parses and validates one named configuration field.
    fn parse(field: &SynField) -> SynResult<Self> {
        let ident = field.ident.clone().expect("named fields always contain an identifier");
        let docs = field
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("doc"))
            .cloned()
            .collect();
        let mut default = None;
        let mut env = None;
        let mut nested = false;
        let mut delimited = false;
        let mut cli = Vec::new();

        for attribute in &field.attrs {
            if !attribute.path().is_ident("argx") {
                continue;
            }
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if default.is_some() {
                        return Err(meta.error("duplicate `default` attribute"));
                    }
                    default = Some(if meta.input.peek(Token![=]) {
                        DefaultValue::Expression(meta.value()?.parse::<Expr>()?)
                    } else {
                        DefaultValue::Trait
                    });
                    return Ok(());
                }
                if meta.path.is_ident("env") {
                    if env.is_some() {
                        return Err(meta.error("duplicate `env` attribute"));
                    }
                    let value = meta.value()?.parse::<LitStr>()?;
                    validate_environment_name(&value, "environment variable name")?;
                    env = Some(value);
                    return Ok(());
                }
                if meta.path.is_ident("flatten") {
                    if nested {
                        return Err(meta.error("duplicate `flatten` attribute"));
                    }
                    nested = true;
                    return Ok(());
                }
                if meta.path.is_ident("long") {
                    if meta.input.peek(Token![=]) {
                        let value = meta.value()?.parse::<LitStr>()?;
                        cli.push(quote!(long = #value));
                    } else {
                        cli.push(quote!(long));
                    }
                    return Ok(());
                }
                if meta.path.is_ident("short") {
                    if meta.input.peek(Token![=]) {
                        let value = meta.value()?.parse::<LitChar>()?;
                        cli.push(quote!(short = #value));
                    } else {
                        cli.push(quote!(short));
                    }
                    return Ok(());
                }
                if meta.path.is_ident("alias") || meta.path.is_ident("help") {
                    let key = &meta.path;
                    let value = meta.value()?.parse::<LitStr>()?;
                    cli.push(quote!(#key = #value));
                    return Ok(());
                }
                if meta.path.is_ident("aliases") {
                    let key = &meta.path;
                    let value = meta.value()?.parse::<syn::ExprArray>()?;
                    cli.push(quote!(#key = #value));
                    return Ok(());
                }
                if meta.path.is_ident("delimited") {
                    delimited = true;
                    cli.push(quote!(delimited));
                    return Ok(());
                }
                if meta.path.is_ident("global")
                    || meta.path.is_ident("value_enum")
                    || meta.path.is_ident("allow_hyphen_values")
                    || meta.path.is_ident("allow_negative_numbers")
                {
                    let key = &meta.path;
                    cli.push(quote!(#key));
                    return Ok(());
                }
                Err(meta.error("unsupported Argx Config field attribute"))
            })?;
        }

        if nested && default.is_some() {
            return Err(SynError::new_spanned(
                field,
                "flattened configuration fields cannot declare a field default",
            ));
        }
        if nested && env.is_some() {
            return Err(SynError::new_spanned(
                field,
                "flattened configuration fields cannot declare an exact environment mapping",
            ));
        }
        let optional = option_inner(&field.ty).is_some();
        if nested && optional {
            return Err(SynError::new_spanned(
                &field.ty,
                "flattened configuration fields must use a concrete Config type",
            ));
        }
        let many = vec_inner(&field.ty).is_some();
        let raw_name = ident.to_string();
        let name = raw_name.strip_prefix("r#").unwrap_or(&raw_name).to_owned();
        Ok(Self {
            ident,
            name,
            ty: field.ty.clone(),
            optional,
            many,
            default,
            env,
            nested,
            delimited,
            cli,
            docs,
        })
    }

    /// Returns whether this field participates in the argv layer.
    pub(crate) const fn exposed_on_cli(&self) -> bool {
        self.nested || !self.cli.is_empty()
    }
}

/// Default-value form declared for a configuration field.
pub(crate) enum DefaultValue {
    /// Uses the field type's `Default` implementation.
    Trait,
    /// Uses an explicit Rust expression.
    Expression(Expr),
}

/// Parses configuration-level `#[argx(...)]` attributes.
fn parse_config_attributes(attributes: &[Attribute]) -> SynResult<Option<LitStr>> {
    let mut prefix = None;
    for attribute in attributes {
        if !attribute.path().is_ident("argx") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                if prefix.is_some() {
                    return Err(meta.error("duplicate `prefix` attribute"));
                }
                let value = meta.value()?.parse::<LitStr>()?;
                validate_environment_name(&value, "environment prefix")?;
                prefix = Some(value);
                Ok(())
            } else {
                Err(meta.error("unsupported Argx Config attribute"))
            }
        })?;
    }
    Ok(prefix)
}

/// Validates an environment variable name or prefix.
fn validate_environment_name(value: &LitStr, label: &str) -> SynResult<()> {
    let text = value.value();
    let mut chars = text.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(SynError::new_spanned(
            value,
            format!(
                "{label} must be a non-empty ASCII identifier containing only letters, digits, and `_`, and cannot start with a digit"
            ),
        ))
    }
}

/// Returns the inner type when `ty` is an `Option<T>`.
pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
    peel(ty, "Option", "option")
}

/// Returns the inner type when `ty` is a `Vec<T>`.
pub(crate) fn vec_inner(ty: &Type) -> Option<&Type> {
    peel(ty, "Vec", "vec")
}

/// Returns the single type argument of a recognized standard container.
fn peel<'a>(ty: &'a Type, name: &str, module: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let segments = &path.path.segments;
    let valid = match segments.len() {
        1 => segments[0].ident == name,
        3 => {
            (segments[0].ident == "std" || segments[0].ident == "core")
                && segments[1].ident == module
                && segments[2].ident == name
        }
        _ => false,
    };
    if !valid {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segments.last()?.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.iter().next()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}
