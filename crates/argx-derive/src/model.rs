//! Semantic model built from a derive input before code generation.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

use crate::{attrs, case};

/// One struct deriving either `Parser` or `Args`.
pub(crate) struct Command {
    /// Rust type name receiving the generated implementations.
    pub ident: syn::Ident,
    /// Generic parameters copied to generated implementations.
    pub generics: syn::Generics,
    /// Whole declaration token stream used to seed stable keys.
    pub fingerprint: String,
    /// Command-line name represented by this declaration.
    pub name: String,
    /// Whether the public `Parser` trait is implemented in addition to `CommandArgs`.
    pub root: bool,
    /// Fields in declaration order.
    pub fields: Vec<Field>,
}

/// One named Rust field and the parse-table entry it contributes.
pub(crate) struct Field {
    /// Source span used for declaration-level diagnostics.
    pub span: Span,
    /// Canonical field name without Rust raw-identifier syntax.
    pub name: String,
    /// Whether the field is named or positional on the command line.
    pub kind: FieldKind,
    /// Syntactic value shape relevant to the static parser table.
    pub shape: Shape,
}

/// Parse-table category of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// A named flag with one or more spellings.
    Flag {
        /// Long spellings without `--`.
        longs: Vec<String>,
        /// Short spellings as ASCII bytes.
        shorts: Vec<u8>,
    },
    /// A positional argument.
    Positional,
}

/// What a field's Rust type says about how many values it can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// `bool`: a switch that does not consume a value.
    Bool,
    /// `Option<T>`: zero or one value.
    Optional,
    /// A bare value type: exactly one value.
    Required,
    /// `Vec<T>` or `Option<Vec<T>>`: zero or more values.
    Many,
}

impl Command {
    /// Parses and validates the subset of a command declaration required by the static model.
    pub(crate) fn from_input(input: &DeriveInput, root: bool) -> syn::Result<Self> {
        let data = match &input.data {
            Data::Struct(data) => data,
            _ => {
                let derive = if root { "Parser" } else { "Args" };
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    format!("{derive} can only be derived for structs"),
                ));
            }
        };

        let fields = match &data.fields {
            Fields::Named(fields) => {
                fields.named.iter().map(Field::from_syn).collect::<syn::Result<Vec<_>>>()?
            }
            Fields::Unit => Vec::new(),
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Parser and Args do not support tuple structs; use named fields",
                ));
            }
        };

        validate_fields(&fields)?;

        let attributes = attrs::command(&input.attrs)?;
        let rust_name = ident_name(&input.ident);
        let name = attributes.name.unwrap_or_else(|| case::to_kebab(&rust_name));
        if name.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "command name cannot be empty"));
        }

        Ok(Self {
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            fingerprint: input.to_token_stream().to_string(),
            name,
            root,
            fields,
        })
    }
}

impl Field {
    /// Converts one named Rust field into static parse metadata.
    fn from_syn(field: &syn::Field) -> syn::Result<Self> {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new_spanned(field, "Parser and Args fields must be named")
        })?;
        let attributes = attrs::field(&field.attrs)?;
        let name = ident_name(&ident);
        let shape = Shape::from_type(&field.ty);

        let kind = if attributes.long.is_some() || attributes.short.is_some() {
            let longs = attributes
                .long
                .map(|long| match long {
                    attrs::Inferred::Infer => case::to_kebab(&name),
                    attrs::Inferred::Explicit(value) => value,
                })
                .into_iter()
                .collect();
            let shorts = attributes
                .short
                .map(|short| match short {
                    attrs::Inferred::Infer => infer_short(&name, ident.span()),
                    attrs::Inferred::Explicit(value) => validate_short(value, ident.span()),
                })
                .transpose()?
                .into_iter()
                .collect();
            FieldKind::Flag { longs, shorts }
        } else {
            FieldKind::Positional
        };

        Ok(Self { span: ident.span(), name, kind, shape })
    }
}

impl Shape {
    /// Infers the value shape from the outer standard collection wrappers.
    fn from_type(ty: &Type) -> Self {
        if type_is(ty, "bool") {
            return Self::Bool;
        }
        if peel(ty, "Option").and_then(|inner| peel(inner, "Vec")).is_some() {
            return Self::Many;
        }
        if peel(ty, "Option").is_some() {
            return Self::Optional;
        }
        if peel(ty, "Vec").is_some() {
            return Self::Many;
        }
        Self::Required
    }
}

/// Peels one standard generic wrapper and returns its first type argument.
fn peel<'a>(ty: &'a Type, container: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != container {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut arguments = arguments.args.iter();
    let GenericArgument::Type(inner) = arguments.next()? else {
        return None;
    };
    if arguments.next().is_some() {
        return None;
    }
    Some(inner)
}

/// Reports whether the last segment of a type path has the requested name.
fn type_is(ty: &Type, expected: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.path.segments.last().is_some_and(|segment| segment.ident == expected)
}

/// Returns an identifier without Rust's raw-identifier prefix.
fn ident_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

/// Infers and validates a short spelling from a Rust field name.
fn infer_short(name: &str, span: Span) -> syn::Result<u8> {
    let character =
        name.chars().next().ok_or_else(|| syn::Error::new(span, "cannot infer short"))?;
    validate_short(character, span)
}

/// Converts one explicit or inferred short spelling to its parser-table byte.
fn validate_short(character: char, span: Span) -> syn::Result<u8> {
    if !character.is_ascii_graphic() || matches!(character, '-' | '=') {
        return Err(syn::Error::new(
            span,
            "short flag must be one visible ASCII character other than `-` or `=`",
        ));
    }
    Ok(character as u8)
}

/// Validates command-wide invariants that cannot be checked one field at a time.
fn validate_fields(fields: &[Field]) -> syn::Result<()> {
    let mut longs: Vec<&str> = Vec::new();
    let mut shorts: Vec<u8> = Vec::new();
    let mut optional_positional_seen = false;
    let mut variadic_positional_span = None;

    for field in fields {
        match &field.kind {
            FieldKind::Flag { longs: field_longs, shorts: field_shorts } => {
                for long in field_longs {
                    validate_long(long, field.span)?;
                    if longs.contains(&long.as_str()) {
                        return Err(syn::Error::new(
                            field.span,
                            format!("duplicate long flag `--{long}`"),
                        ));
                    }
                    longs.push(long.as_str());
                }
                for short in field_shorts {
                    if shorts.contains(short) {
                        return Err(syn::Error::new(
                            field.span,
                            format!("duplicate short flag `-{}`", char::from(*short)),
                        ));
                    }
                    shorts.push(*short);
                }
            }
            FieldKind::Positional => {
                if let Some(span) = variadic_positional_span {
                    return Err(syn::Error::new(
                        span,
                        "variadic positional argument must be the last positional argument",
                    ));
                }

                let required = matches!(field.shape, Shape::Bool | Shape::Required);
                if required && optional_positional_seen {
                    return Err(syn::Error::new(
                        field.span,
                        "required positional arguments cannot follow optional positional arguments",
                    ));
                }
                if !required {
                    optional_positional_seen = true;
                }
                if field.shape == Shape::Many {
                    variadic_positional_span = Some(field.span);
                }
            }
        }
    }

    Ok(())
}

/// Validates one explicit or inferred long spelling.
fn validate_long(long: &str, span: Span) -> syn::Result<()> {
    if long.is_empty()
        || long.starts_with('-')
        || long.contains('=')
        || long.chars().any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(syn::Error::new(
            span,
            "long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls",
        ));
    }
    Ok(())
}
