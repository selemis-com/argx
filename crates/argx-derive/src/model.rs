//! Canonical semantic model built from derive input before code generation.
//!
//! CLI meaning is normalized here exactly once. Code generation then projects that meaning into
//! private runtime command tables while Rust-specific construction and value conversion remain in
//! separate binding data. Future help, contract, and completion projections should extend this
//! model rather than reinterpret attributes or runtime parser tables.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields, GenericParam, Type, visit::Visit as _};

use crate::{attrs, case};

mod shape;

pub(crate) use shape::Shape;
use shape::{peel_option, peel_vec, rendered_path, validate_value_shape};

/// One normalized command declaration.
pub(crate) struct Command {
    /// Rust-facing information required to implement the derived type.
    pub binding: CommandBinding,
    /// Command-line semantics shared by every generated projection.
    pub semantics: CommandSemantics,
    /// Fields in declaration order.
    pub fields: Vec<Field>,
}

/// Rust-facing information for a `Parser` or `Args` declaration.
pub(crate) struct CommandBinding {
    /// Rust type name receiving the generated implementations.
    pub ident: syn::Ident,
    /// Generic parameters copied to generated implementations.
    pub generics: syn::Generics,
    /// Whole declaration token stream used to seed stable semantic identities.
    pub fingerprint: String,
    /// Whether the public `Parser` trait is implemented in addition to `CommandArgs`.
    pub root: bool,
    /// Whether the declaration is a unit struct.
    pub unit: bool,
}

/// CLI semantics common to root commands and selectable subcommands.
pub(crate) struct CommandSemantics {
    /// Command-line name represented by this declaration.
    pub name: String,
    /// One-line help summary for this command.
    pub about: Option<String>,
}

/// One enum deriving `Subcommand`.
pub(crate) struct Subcommand {
    /// Rust-facing information required to implement the enum.
    pub binding: SubcommandBinding,
    /// Variants in declaration order.
    pub variants: Vec<Variant>,
}

/// Rust-facing information for a `Subcommand` declaration.
pub(crate) struct SubcommandBinding {
    /// Rust enum receiving the generated implementation.
    pub ident: syn::Ident,
    /// Generic parameters copied to generated implementations.
    pub generics: syn::Generics,
    /// Whole declaration token stream used to seed stable variant identities.
    pub fingerprint: String,
}

/// One selectable subcommand variant.
pub(crate) struct Variant {
    /// Rust-facing variant construction information.
    pub binding: VariantBinding,
    /// Command-line semantics of this selectable command.
    pub semantics: CommandSemantics,
}

/// Rust-facing information for one subcommand variant.
pub(crate) struct VariantBinding {
    /// Rust enum variant name.
    pub ident: syn::Ident,
    /// Optional reusable `Args` payload.
    pub payload: Option<Type>,
}

/// One named Rust field after CLI semantics have been normalized.
pub(crate) struct Field {
    /// Rust-facing information used to construct the destination value.
    pub binding: FieldBinding,
    /// CLI meaning of this field.
    pub semantics: FieldSemantics,
}

/// Rust-facing information for one destination field.
pub(crate) struct FieldBinding {
    /// Rust field identifier used when generated code builds the destination value.
    pub ident: syn::Ident,
    /// Declared Rust field type.
    pub ty: Type,
    /// Source span used for declaration-level diagnostics.
    pub span: Span,
    /// Field name without Rust raw-identifier syntax, used by binding diagnostics.
    pub name: String,
    /// Normalized typed-value conversion, when this field binds CLI values directly.
    pub value: Option<ValueBinding>,
}

/// Rust conversion information for a value-bearing field.
pub(crate) struct ValueBinding {
    /// Rust type receiving one parsed value.
    pub ty: Type,
    /// Conversion strategy selected from the destination type.
    pub conversion: ValueConversion,
    /// Whether a repeated field preserves absence with an outer `Option`.
    pub optional_collection: bool,
}

/// Conversion strategy for one raw CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueConversion {
    /// Preserve UTF-8 text directly as `String`.
    Text,
    /// Reconstruct an operating-system string before converting the destination.
    Os,
    /// Parse UTF-8 text through `FromStr`.
    FromStr,
}

/// CLI role represented by one Rust field.
pub(crate) enum FieldSemantics {
    /// One named or positional CLI argument.
    Argument(Argument),
    /// Another independently derived `Args` declaration composed inline.
    Flatten,
    /// A required nested command selected from a derived subcommand enum.
    Subcommand,
}

/// Normalized semantics for one named or positional argument.
pub(crate) struct Argument {
    /// One-line help summary for this argument.
    pub help: Option<String>,
    /// Whether the argument is named or positional on the command line.
    pub kind: ArgumentKind,
    /// Syntactic value shape relevant to CLI cardinality.
    pub shape: Shape,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
}

/// Command-line category of one argument.
pub(crate) enum ArgumentKind {
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

impl Command {
    /// Parses and validates one command into the canonical derive-time model.
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
        validate_composed_generics(&fields, &input.generics)?;

        let attributes = attrs::command(&input.attrs)?;
        let rust_name = ident_name(&input.ident);
        let name = attributes.name.unwrap_or_else(|| case::to_kebab(&rust_name));
        if name.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "command name cannot be empty"));
        }
        let about = attributes.about.or_else(|| attrs::doc_summary(&input.attrs));

        Ok(Self {
            binding: CommandBinding {
                ident: input.ident.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
                root,
                unit,
            },
            semantics: CommandSemantics { name, about },
            fields,
        })
    }
}

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

        let mut names = Vec::<String>::new();
        let mut variants = Vec::with_capacity(data.variants.len());
        for variant in &data.variants {
            let attributes = attrs::variant(&variant.attrs)?;
            let rust_name = ident_name(&variant.ident);
            let name = attributes.name.unwrap_or_else(|| case::to_kebab(&rust_name));
            let about = attributes.about.or_else(|| attrs::doc_summary(&variant.attrs));
            validate_subcommand_name(&name, variant.ident.span())?;
            if names.contains(&name) {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    format!("duplicate subcommand `{name}`"),
                ));
            }
            names.push(name.clone());

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
                semantics: CommandSemantics { name, about },
            });
        }

        validate_variant_generics(&variants, &input.generics)?;

        Ok(Self {
            binding: SubcommandBinding {
                ident: input.ident.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
            },
            variants,
        })
    }
}

impl Field {
    /// Converts one named Rust field into normalized CLI semantics plus Rust binding information.
    fn from_syn(field: &syn::Field) -> syn::Result<Self> {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new_spanned(field, "Parser and Args fields must be named")
        })?;
        let attributes = attrs::field(&field.attrs)?;
        let name = ident_name(&ident);
        let mut binding =
            FieldBinding { span: ident.span(), ident, ty: field.ty.clone(), name, value: None };

        if attributes.flatten && attributes.subcommand {
            return Err(syn::Error::new(
                binding.span,
                "`flatten` and `subcommand` cannot be combined",
            ));
        }

        if attributes.subcommand {
            if attributes.long.is_some()
                || attributes.short.is_some()
                || attributes.allow_hyphen_values
                || attributes.allow_negative_numbers
                || attributes.help.is_some()
            {
                return Err(syn::Error::new(
                    binding.span,
                    "`subcommand` cannot be combined with flag, value, or help attributes",
                ));
            }
            if peel_option(&binding.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &binding.ty,
                    "`subcommand` does not support `Option<T>`; hold the Subcommand enum directly",
                ));
            }
            if peel_vec(&binding.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &binding.ty,
                    "`subcommand` does not support collection wrappers",
                ));
            }
            return Ok(Self { binding, semantics: FieldSemantics::Subcommand });
        }

        if attributes.flatten {
            if attributes.long.is_some()
                || attributes.short.is_some()
                || attributes.allow_hyphen_values
                || attributes.allow_negative_numbers
                || attributes.help.is_some()
            {
                return Err(syn::Error::new(
                    binding.span,
                    "`flatten` cannot be combined with flag, value, or help attributes",
                ));
            }
            if peel_option(&binding.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &binding.ty,
                    "`flatten` does not support `Option<T>`; hold the Args struct directly",
                ));
            }
            if peel_vec(&binding.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &binding.ty,
                    "`flatten` does not support collection wrappers; hold one Args struct directly",
                ));
            }

            return Ok(Self { binding, semantics: FieldSemantics::Flatten });
        }

        let shape = Shape::from_type(&binding.ty);
        validate_value_shape(&binding.ty, shape, binding.span)?;

        let kind = if attributes.long.is_some() || attributes.short.is_some() {
            let longs = attributes
                .long
                .map(|long| match long {
                    attrs::Inferred::Infer => case::to_kebab(&binding.name),
                    attrs::Inferred::Explicit(value) => value,
                })
                .into_iter()
                .collect();
            let shorts = attributes
                .short
                .map(|short| match short {
                    attrs::Inferred::Infer => infer_short(&binding.name, binding.span),
                    attrs::Inferred::Explicit(value) => validate_short(value, binding.span),
                })
                .transpose()?
                .into_iter()
                .collect();
            ArgumentKind::Flag { longs, shorts }
        } else {
            ArgumentKind::Positional
        };

        if attributes.allow_hyphen_values && matches!(&kind, ArgumentKind::Positional) {
            return Err(syn::Error::new(
                binding.span,
                "`allow_hyphen_values` is only valid on named flags",
            ));
        }
        if (attributes.allow_hyphen_values || attributes.allow_negative_numbers)
            && shape == Shape::Bool
        {
            return Err(syn::Error::new(
                binding.span,
                "value policies are not valid on bool fields",
            ));
        }

        let switch = matches!(&kind, ArgumentKind::Flag { .. }) && shape == Shape::Bool;
        if !switch {
            binding.value = Some(value_binding(&binding.ty, shape));
        }

        let help = attributes.help.or_else(|| attrs::doc_summary(&field.attrs));
        Ok(Self {
            binding,
            semantics: FieldSemantics::Argument(Argument {
                help,
                kind,
                shape,
                allow_hyphen_values: attributes.allow_hyphen_values,
                allow_negative_numbers: attributes.allow_negative_numbers,
            }),
        })
    }

    /// Returns this field's normalized argument semantics, if it is a CLI argument.
    pub(crate) const fn argument(&self) -> Option<&Argument> {
        match &self.semantics {
            FieldSemantics::Argument(argument) => Some(argument),
            FieldSemantics::Flatten | FieldSemantics::Subcommand => None,
        }
    }

    /// Reports whether this field composes another `Args` declaration.
    pub(crate) const fn is_flatten(&self) -> bool {
        matches!(&self.semantics, FieldSemantics::Flatten)
    }

    /// Reports whether this field is a value-less boolean flag.
    pub(crate) fn is_switch(&self) -> bool {
        self.argument().is_some_and(|argument| {
            matches!(&argument.kind, ArgumentKind::Flag { .. }) && argument.shape == Shape::Bool
        })
    }

    /// Returns normalized typed-value binding information.
    pub(crate) const fn value_binding(&self) -> &ValueBinding {
        self.binding.value.as_ref().expect("composed fields and switches do not bind typed values")
    }
}

/// Normalizes Rust conversion behavior for one CLI value-bearing field.
fn value_binding(ty: &Type, shape: Shape) -> ValueBinding {
    let value_ty = match shape {
        Shape::Bool | Shape::Required => ty,
        Shape::Optional => {
            peel_option(ty).expect("optional shape must contain an Option value type")
        }
        Shape::Many => {
            let collection = peel_option(ty).unwrap_or(ty);
            peel_vec(collection).expect("many shape must contain a Vec item type")
        }
    };
    let rendered = rendered_path(value_ty);
    let conversion = if matches!(
        rendered.as_str(),
        "String"
            | "std::string::String"
            | "::std::string::String"
            | "alloc::string::String"
            | "::alloc::string::String"
    ) {
        ValueConversion::Text
    } else if matches!(
        rendered.as_str(),
        "OsString"
            | "std::ffi::OsString"
            | "::std::ffi::OsString"
            | "PathBuf"
            | "std::path::PathBuf"
            | "::std::path::PathBuf"
    ) {
        ValueConversion::Os
    } else {
        ValueConversion::FromStr
    };

    ValueBinding {
        ty: value_ty.clone(),
        conversion,
        optional_collection: shape == Shape::Many && peel_option(ty).is_some(),
    }
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
    let mut subcommand_seen = false;

    for field in fields {
        match &field.semantics {
            FieldSemantics::Argument(Argument {
                kind: ArgumentKind::Flag { longs: field_longs, shorts: field_shorts },
                ..
            }) => {
                for long in field_longs {
                    validate_long(long, field.binding.span)?;
                    if long == "help" {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`--help` is reserved by Argx",
                        ));
                    }
                    if longs.contains(&long.as_str()) {
                        return Err(syn::Error::new(
                            field.binding.span,
                            format!("duplicate long flag `--{long}`"),
                        ));
                    }
                    longs.push(long.as_str());
                }
                for short in field_shorts {
                    if *short == b'h' {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`-h` is reserved by Argx",
                        ));
                    }
                    if shorts.contains(short) {
                        return Err(syn::Error::new(
                            field.binding.span,
                            format!("duplicate short flag `-{}`", char::from(*short)),
                        ));
                    }
                    shorts.push(*short);
                }
            }
            FieldSemantics::Argument(Argument {
                kind: ArgumentKind::Positional, shape, ..
            }) => {
                if let Some(span) = variadic_positional_span {
                    return Err(syn::Error::new(
                        span,
                        "variadic positional argument must be the last positional argument",
                    ));
                }

                let required = matches!(*shape, Shape::Bool | Shape::Required);
                if required && optional_positional_seen {
                    return Err(syn::Error::new(
                        field.binding.span,
                        "required positional arguments cannot follow optional positional arguments",
                    ));
                }
                if !required {
                    optional_positional_seen = true;
                }
                if *shape == Shape::Many {
                    variadic_positional_span = Some(field.binding.span);
                }
            }
            FieldSemantics::Flatten => {}
            FieldSemantics::Subcommand => {
                if subcommand_seen {
                    return Err(syn::Error::new(
                        field.binding.span,
                        "a command can contain only one `subcommand` field",
                    ));
                }
                subcommand_seen = true;
            }
        }
    }

    Ok(())
}

/// Rejects composed types that depend on the containing declaration's generic parameters.
///
/// Flattened tables and subcommand tables are materialized statically. A concrete generic child
/// such as `Shared<String>` is fine, but a child whose type still depends on `T`, `'a`, or a const
/// parameter cannot be named from that static initializer on stable Rust.
fn validate_composed_generics(fields: &[Field], generics: &syn::Generics) -> syn::Result<()> {
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
        let attribute = match &field.semantics {
            FieldSemantics::Flatten => "flatten",
            FieldSemantics::Subcommand => "subcommand",
            FieldSemantics::Argument(_) => continue,
        };
        let ty = &field.binding.ty;
        let mut visitor = GenericUse { params: &params, found: false };
        visitor.visit_type(ty);
        if visitor.found {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "`{attribute}` cannot depend on the containing struct's generic parameters; use a concrete derived type"
                ),
            ));
        }
    }
    Ok(())
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

/// One generic parameter name relevant while inspecting a composed field type.
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

#[cfg(test)]
mod tests {
    use syn::{DeriveInput, parse_quote};

    use super::{ArgumentKind, Command, FieldSemantics, Shape, ValueConversion};

    #[test]
    fn command_model_separates_cli_semantics_from_rust_binding() {
        let input: DeriveInput = parse_quote! {
            /// Example command.
            #[argx(name = "example")]
            struct Cli {
                /// Enable verbose output.
                #[argx(short, long)]
                verbose: bool,
                output: Option<std::path::PathBuf>,
                #[argx(flatten)]
                shared: Shared,
                #[argx(subcommand)]
                command: Commands,
            }
        };

        let command = Command::from_input(&input, true).expect("command model should be valid");
        assert_eq!(command.semantics.name, "example");
        assert_eq!(command.semantics.about.as_deref(), Some("Example command."));
        assert!(command.binding.root);

        let verbose = &command.fields[0];
        let Some(argument) = verbose.argument() else {
            panic!("verbose should be an argument");
        };
        assert!(matches!(&argument.kind, ArgumentKind::Flag { .. }));
        assert_eq!(argument.shape, Shape::Bool);
        assert!(verbose.binding.value.is_none());

        let output = &command.fields[1];
        let Some(argument) = output.argument() else {
            panic!("output should be an argument");
        };
        assert!(matches!(&argument.kind, ArgumentKind::Positional));
        assert_eq!(argument.shape, Shape::Optional);
        assert_eq!(output.value_binding().conversion, ValueConversion::Os);

        assert!(matches!(&command.fields[2].semantics, FieldSemantics::Flatten));
        assert!(command.fields[2].binding.value.is_none());
        assert!(matches!(&command.fields[3].semantics, FieldSemantics::Subcommand));
        assert!(command.fields[3].binding.value.is_none());
    }
}
