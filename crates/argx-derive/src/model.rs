//! Semantic model built from a derive input before code generation.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{
    Data, DeriveInput, Fields, GenericArgument, GenericParam, PathArguments, Type,
    visit::Visit as _,
};

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
    /// Whether the declaration is a unit struct.
    pub unit: bool,
    /// Fields in declaration order.
    pub fields: Vec<Field>,
}

/// One named Rust field and the parse-table entry it contributes.
pub(crate) struct Field {
    /// Rust field identifier used when generated code builds the destination value.
    pub ident: syn::Ident,
    /// Declared Rust field type.
    pub ty: Type,
    /// Source span used for declaration-level diagnostics.
    pub span: Span,
    /// Canonical field name without Rust raw-identifier syntax.
    pub name: String,
    /// Whether the field is named or positional on the command line.
    pub kind: FieldKind,
    /// Syntactic value shape relevant to binding cardinality.
    pub shape: Shape,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
}

/// Parse-table category of a field.
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
    /// Another independently derived `Args` declaration composed inline.
    Flatten {
        /// Child declaration type.
        ty: Type,
    },
}

/// What a field's Rust type says about how many values it can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// Bare `bool`, which is a switch when used as a flag.
    Bool,
    /// `Option<T>`: zero or one value.
    Optional,
    /// A bare value type: exactly one value.
    Required,
    /// `Vec<T>` or `Option<Vec<T>>`: zero or more values.
    Many,
}

impl Command {
    /// Parses and validates the subset of a command declaration required by the typed parser.
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

        let unit = matches!(&data.fields, Fields::Unit);
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
        validate_flatten_generics(&fields, &input.generics)?;

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
            unit,
            fields,
        })
    }
}

impl Field {
    /// Converts one named Rust field into static parse and typed-binding metadata.
    fn from_syn(field: &syn::Field) -> syn::Result<Self> {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new_spanned(field, "Parser and Args fields must be named")
        })?;
        let attributes = attrs::field(&field.attrs)?;
        let name = ident_name(&ident);

        if attributes.flatten {
            if attributes.long.is_some()
                || attributes.short.is_some()
                || attributes.allow_hyphen_values
                || attributes.allow_negative_numbers
            {
                return Err(syn::Error::new(
                    ident.span(),
                    "`flatten` cannot be combined with flag or value attributes",
                ));
            }
            if peel_option(&field.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`flatten` does not support `Option<T>`; hold the Args struct directly",
                ));
            }
            if peel_vec(&field.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "`flatten` does not support collection wrappers; hold one Args struct directly",
                ));
            }

            return Ok(Self {
                span: ident.span(),
                ident,
                ty: field.ty.clone(),
                name,
                kind: FieldKind::Flatten { ty: field.ty.clone() },
                // Flattened fields do not bind a value themselves. The shape is never inspected
                // by typed conversion and stays required only as an internal placeholder.
                shape: Shape::Required,
                allow_hyphen_values: false,
                allow_negative_numbers: false,
            });
        }

        let shape = Shape::from_type(&field.ty);
        validate_value_shape(&field.ty, shape, ident.span())?;

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

        if attributes.allow_hyphen_values && matches!(&kind, FieldKind::Positional) {
            return Err(syn::Error::new(
                ident.span(),
                "`allow_hyphen_values` is only valid on named flags",
            ));
        }
        if (attributes.allow_hyphen_values || attributes.allow_negative_numbers)
            && shape == Shape::Bool
        {
            return Err(syn::Error::new(
                ident.span(),
                "value policies are not valid on bool fields",
            ));
        }

        Ok(Self {
            span: ident.span(),
            ident,
            ty: field.ty.clone(),
            name,
            kind,
            shape,
            allow_hyphen_values: attributes.allow_hyphen_values,
            allow_negative_numbers: attributes.allow_negative_numbers,
        })
    }

    /// Reports whether this field composes another `Args` declaration.
    pub(crate) const fn is_flatten(&self) -> bool {
        matches!(self.kind, FieldKind::Flatten { .. })
    }

    /// Reports whether this field is a value-less boolean flag.
    pub(crate) fn is_switch(&self) -> bool {
        matches!(&self.kind, FieldKind::Flag { .. }) && self.shape == Shape::Bool
    }

    /// Returns the Rust type receiving one parsed value.
    pub(crate) fn value_type(&self) -> &Type {
        assert!(!self.is_flatten(), "flattened fields do not have a scalar value type");
        match self.shape {
            Shape::Bool | Shape::Required => &self.ty,
            Shape::Optional => {
                peel_option(&self.ty).expect("optional shape must contain an Option value type")
            }
            Shape::Many => {
                let collection = peel_option(&self.ty).unwrap_or(&self.ty);
                peel_vec(collection).expect("many shape must contain a Vec item type")
            }
        }
    }

    /// Reports whether a repeated field preserves absence with an outer `Option`.
    pub(crate) fn optional_collection(&self) -> bool {
        self.shape == Shape::Many && peel_option(&self.ty).is_some()
    }

    /// Reports whether one value is the standard UTF-8 string type.
    pub(crate) fn string_value(&self) -> bool {
        matches!(
            rendered_path(self.value_type()).as_str(),
            "String"
                | "std::string::String"
                | "::std::string::String"
                | "alloc::string::String"
                | "::alloc::string::String"
        )
    }

    /// Reports whether one value should be reconstructed as an operating-system string.
    pub(crate) fn os_value(&self) -> bool {
        matches!(
            rendered_path(self.value_type()).as_str(),
            "OsString"
                | "std::ffi::OsString"
                | "::std::ffi::OsString"
                | "PathBuf"
                | "std::path::PathBuf"
                | "::std::path::PathBuf"
        )
    }
}

impl Shape {
    /// Infers the value shape from the outer standard collection wrappers.
    fn from_type(ty: &Type) -> Self {
        if matches!(
            rendered_path(ty).as_str(),
            "bool"
                | "std::primitive::bool"
                | "::std::primitive::bool"
                | "core::primitive::bool"
                | "::core::primitive::bool"
        ) {
            return Self::Bool;
        }
        if peel_option(ty).and_then(peel_vec).is_some() {
            return Self::Many;
        }
        if peel_option(ty).is_some() {
            return Self::Optional;
        }
        if peel_vec(ty).is_some() {
            return Self::Many;
        }
        Self::Required
    }
}

/// Rejects collection nestings that the typed binding model does not define.
fn validate_value_shape(ty: &Type, shape: Shape, span: Span) -> syn::Result<()> {
    let value = match shape {
        Shape::Bool | Shape::Required => return Ok(()),
        Shape::Optional => peel_option(ty).expect("optional shape must contain Option"),
        Shape::Many => {
            let collection = peel_option(ty).unwrap_or(ty);
            peel_vec(collection).expect("many shape must contain Vec")
        }
    };

    if peel_option(value).is_some() || peel_vec(value).is_some() {
        return Err(syn::Error::new(
            span,
            "nested Option and Vec value wrappers are not supported",
        ));
    }
    Ok(())
}

/// Peels one standard `Option` wrapper.
fn peel_option(ty: &Type) -> Option<&Type> {
    peel_standard(
        ty,
        &[
            "Option",
            "std::option::Option",
            "::std::option::Option",
            "core::option::Option",
            "::core::option::Option",
        ],
    )
}

/// Peels one standard `Vec` wrapper.
fn peel_vec(ty: &Type) -> Option<&Type> {
    peel_standard(
        ty,
        &["Vec", "std::vec::Vec", "::std::vec::Vec", "alloc::vec::Vec", "::alloc::vec::Vec"],
    )
}

/// Peels one recognized standard generic wrapper and returns its sole type argument.
fn peel_standard<'a>(ty: &'a Type, accepted: &[&str]) -> Option<&'a Type> {
    if !accepted.contains(&rendered_path(ty).split('<').next()?) {
        return None;
    }

    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
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

/// Returns a type path as written with token-stream spacing removed.
fn rendered_path(ty: &Type) -> String {
    ty.to_token_stream().to_string().replace(' ', "")
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
            FieldKind::Flatten { .. } => {}
        }
    }

    Ok(())
}

/// Rejects flattened types that depend on the containing declaration's generic parameters.
///
/// Flattened tables are materialized as one static command table. A concrete generic child such
/// as `Shared<String>` is fine, but a child whose type still depends on `T`, `'a`, or a const
/// parameter cannot be named from that static initializer on stable Rust.
fn validate_flatten_generics(fields: &[Field], generics: &syn::Generics) -> syn::Result<()> {
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

    for field in fields {
        let FieldKind::Flatten { ty } = &field.kind else {
            continue;
        };
        let mut visitor = GenericUse { params: &params, found: false };
        visitor.visit_type(ty);
        if visitor.found {
            return Err(syn::Error::new_spanned(
                ty,
                "`flatten` cannot depend on the containing struct's generic parameters; use a concrete Args type",
            ));
        }
    }
    Ok(())
}

/// One generic parameter name relevant while inspecting a flattened field type.
#[derive(Debug)]
enum GenericName {
    /// Type or const parameter identifier.
    Ident(syn::Ident),
    /// Lifetime parameter identifier without the apostrophe.
    Lifetime(syn::Ident),
}

/// Visitor that detects use of one containing generic parameter inside a flattened type.
#[derive(Debug)]
struct GenericUse<'a> {
    /// Generic names declared by the containing struct.
    params: &'a [GenericName],
    /// Whether a matching parameter was encountered.
    found: bool,
}

impl<'ast> syn::visit::Visit<'ast> for GenericUse<'_> {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if let Some(first) = path.path.segments.first()
            && self
                .params
                .iter()
                .any(|param| matches!(param, GenericName::Ident(name) if name == &first.ident))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_type_path(self, path);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(first) = path.path.segments.first()
            && self
                .params
                .iter()
                .any(|param| matches!(param, GenericName::Ident(name) if name == &first.ident))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if self
            .params
            .iter()
            .any(|param| matches!(param, GenericName::Lifetime(name) if name == &lifetime.ident))
        {
            self.found = true;
        }
    }
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
