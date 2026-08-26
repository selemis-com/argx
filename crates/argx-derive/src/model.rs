//! Canonical semantic model built from derive input before code generation.
//!
//! CLI meaning is normalized here exactly once. Code generation then projects that meaning into
//! shared static command tables while Rust-specific construction, value conversion, and semantic
//! type resolution remain separate. Future projections should extend this model rather than
//! reinterpret attributes or generated command tables.
//!
//! Validation is split between declaration-local checks that the proc macro can resolve directly
//! and composition checks emitted into generated constants. The latter are necessary for flattened
//! `Args` declarations because one macro expansion cannot introspect another expansion's fields.

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
    /// Visibility of the derived declaration.
    pub visibility: syn::Visibility,
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
    /// Full prose rendered before generated command help sections.
    pub description: Option<String>,
    /// User-authored help sections from Rust doc comments.
    pub help_sections: Vec<HelpSection>,
    /// Short version text expression.
    pub version: Option<syn::Expr>,
    /// Long version text expression.
    pub long_version: Option<syn::Expr>,
    /// Hidden command spellings accepted in addition to the canonical name.
    pub aliases: Vec<String>,
}

/// One user-authored command help section.
pub(crate) struct HelpSection {
    /// Section heading.
    pub heading: String,
    /// Section body.
    pub body: String,
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
    /// Visibility of the derived declaration.
    pub visibility: syn::Visibility,
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
    /// Help-section heading applied when this field flattens another `Args` declaration.
    pub help_heading: Option<String>,
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
    /// Typed Rust expression used when this argument is absent.
    pub default: Option<syn::Expr>,
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
    /// Canonical user-facing label used by diagnostics.
    pub diagnostic: String,
    /// Whether a named argument remains in scope for descendant commands.
    pub global: bool,
    /// Syntactic value shape relevant to CLI cardinality.
    pub shape: Shape,
    /// Environment variable consulted when argv does not supply this argument.
    pub env: Option<String>,
    /// Whether absence is satisfied by a typed Rust default.
    pub has_default: bool,
    /// Field names that must be satisfied when this argument is supplied.
    pub requires: Vec<String>,
    /// Field names that cannot be supplied together with this argument.
    pub conflicts: Vec<String>,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
}

/// Command-line category of one argument.
pub(crate) enum ArgumentKind {
    /// A named flag with one or more spellings.
    Flag {
        /// Canonical long spellings without `--`.
        longs: Vec<String>,
        /// Hidden long aliases without `--`.
        aliases: Vec<String>,
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

        // Normalize every field before validating relationships between fields. This guarantees
        // later validation and code generation never need to inspect raw `syn::Field` attributes.
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

        // Command-level syntax is normalized independently from field syntax. Root-only and
        // subcommand-only policy is enforced here so the canonical model cannot represent it.
        let attributes = attrs::command(&input.attrs)?;
        if !attributes.aliases.is_empty() {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "command aliases are only valid on Subcommand variants",
            ));
        }
        if !root && (attributes.version.is_some() || attributes.long_version.is_some()) {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "version metadata is only valid on Parser declarations and Subcommand variants",
            ));
        }
        // Validate every invariant the current macro expansion can see. Cross-flatten invariants
        // are emitted later as const assertions over the composed child tables.
        let has_version = attributes.version.is_some() || attributes.long_version.is_some();
        validate_fields(&fields, has_version)?;
        validate_constraints(&fields)?;
        validate_composed_generics(&fields, &input.generics)?;

        // Only after validation do we derive human-facing metadata. This keeps inferred names and
        // doc-derived help in the same semantic representation as explicit attribute overrides.
        let rust_name = ident_name(&input.ident);
        let name = attributes.name.unwrap_or_else(|| case::to_kebab(&rust_name));
        if name.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "command name cannot be empty"));
        }
        let docs = attrs::doc_help(&input.attrs);
        let about = attributes.about.clone().or(docs.summary);
        let description = attributes.about.or(docs.description);
        let help_sections = docs
            .sections
            .into_iter()
            .map(|section| HelpSection { heading: section.heading, body: section.body })
            .collect();
        let version = attributes.version;
        let long_version = attributes.long_version;

        Ok(Self {
            binding: CommandBinding {
                ident: input.ident.clone(),
                visibility: input.vis.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
                root,
                unit,
            },
            semantics: CommandSemantics {
                name,
                about,
                description,
                help_sections,
                version,
                long_version,
                aliases: Vec::new(),
            },
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

impl Field {
    /// Converts one named Rust field into normalized CLI semantics plus Rust binding information.
    fn from_syn(field: &syn::Field) -> syn::Result<Self> {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new_spanned(field, "Parser and Args fields must be named")
        })?;
        let attributes = attrs::field(&field.attrs)?;
        let name = ident_name(&ident);
        let mut binding = FieldBinding {
            span: ident.span(),
            ident,
            ty: field.ty.clone(),
            name,
            value: None,
            default: None,
        };

        if attributes.flatten && attributes.subcommand {
            return Err(syn::Error::new(
                binding.span,
                "`flatten` and `subcommand` cannot be combined",
            ));
        }
        if (attributes.flatten || attributes.subcommand)
            && (!attributes.requires.is_empty() || !attributes.conflicts.is_empty())
        {
            return Err(syn::Error::new(
                binding.span,
                "`requires` and `conflicts` are only valid on argument fields",
            ));
        }

        // Structural fields never bind CLI values themselves. Reject value metadata before any
        // type-shape inference so unsupported combinations cannot leak into later projections.
        if attributes.subcommand {
            if attributes.long.is_some()
                || attributes.short.is_some()
                || !attributes.aliases.is_empty()
                || attributes.global
                || attributes.env.is_some()
                || attributes.default.is_some()
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
            return Ok(Self { binding, semantics: FieldSemantics::Subcommand, help_heading: None });
        }

        if attributes.flatten {
            if attributes.long.is_some()
                || attributes.short.is_some()
                || !attributes.aliases.is_empty()
                || attributes.global
                || attributes.env.is_some()
                || attributes.default.is_some()
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

            return Ok(Self {
                binding,
                semantics: FieldSemantics::Flatten,
                help_heading: attrs::doc_summary(&field.attrs),
            });
        }

        // Ordinary fields are classified once into cardinality, command-line kind, and conversion
        // strategy. Code generation consumes these facts directly rather than repeating type tests.
        let shape = Shape::from_type(&binding.ty);
        validate_value_shape(&binding.ty, shape, binding.span)?;

        let named = attributes.long.is_some() || attributes.short.is_some();
        if !named && !attributes.aliases.is_empty() {
            return Err(syn::Error::new(
                binding.span,
                "`alias` and `aliases` are only valid on named flags",
            ));
        }
        let kind = if named {
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
            ArgumentKind::Flag { longs, aliases: attributes.aliases, shorts }
        } else {
            ArgumentKind::Positional
        };

        if attributes.allow_hyphen_values && matches!(&kind, ArgumentKind::Positional) {
            return Err(syn::Error::new(
                binding.span,
                "`allow_hyphen_values` is only valid on named flags",
            ));
        }
        if attributes.global && matches!(&kind, ArgumentKind::Positional) {
            return Err(syn::Error::new(binding.span, "`global` is only valid on named flags"));
        }
        if (attributes.allow_hyphen_values || attributes.allow_negative_numbers)
            && shape == Shape::Bool
        {
            return Err(syn::Error::new(
                binding.span,
                "value policies are not valid on bool fields",
            ));
        }
        if attributes.env.is_some()
            && (!matches!(&kind, ArgumentKind::Flag { .. })
                || matches!(shape, Shape::Bool | Shape::Many))
        {
            return Err(syn::Error::new(
                binding.span,
                "`env` is only supported on scalar value-taking flags",
            ));
        }
        let env = attributes
            .env
            .map(|env| {
                validate_env_name(&env.value(), env.span())?;
                Ok::<_, syn::Error>(env.value())
            })
            .transpose()?;
        if attributes.default.is_some()
            && (!matches!(&kind, ArgumentKind::Flag { .. })
                || matches!(shape, Shape::Bool | Shape::Many))
        {
            return Err(syn::Error::new(
                binding.span,
                "`default` is only supported on scalar value-taking flags",
            ));
        }

        let diagnostic = match &kind {
            ArgumentKind::Flag { longs, shorts, .. } => longs.first().map_or_else(
                || {
                    shorts.first().map_or_else(
                        || binding.name.clone(),
                        |short| format!("-{}", char::from(*short)),
                    )
                },
                |long| format!("--{long}"),
            ),
            ArgumentKind::Positional => binding.name.clone(),
        };
        let has_default = attributes.default.is_some();
        let switch = matches!(&kind, ArgumentKind::Flag { .. }) && shape == Shape::Bool;
        if !switch {
            binding.value = Some(value_binding(&binding.ty, shape));
        }
        binding.default = attributes.default;

        let help = attributes.help.or_else(|| attrs::doc_summary(&field.attrs));
        Ok(Self {
            binding,
            semantics: FieldSemantics::Argument(Argument {
                help,
                kind,
                diagnostic,
                global: attributes.global,
                shape,
                env,
                has_default,
                requires: attributes.requires.into_iter().map(|value| value.value()).collect(),
                conflicts: attributes.conflicts.into_iter().map(|value| value.value()).collect(),
                allow_hyphen_values: attributes.allow_hyphen_values,
                allow_negative_numbers: attributes.allow_negative_numbers,
            }),
            help_heading: None,
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
fn validate_fields(fields: &[Field], has_version: bool) -> syn::Result<()> {
    let mut longs: Vec<&str> = Vec::new();
    let mut shorts: Vec<u8> = Vec::new();
    let mut optional_positional_seen = false;
    let mut variadic_positional_span = None;
    let mut subcommand_seen = false;

    for field in fields {
        match &field.semantics {
            FieldSemantics::Argument(Argument {
                kind:
                    ArgumentKind::Flag {
                        longs: field_longs,
                        aliases: field_aliases,
                        shorts: field_shorts,
                    },
                ..
            }) => {
                for long in field_longs.iter().chain(field_aliases) {
                    validate_long(long, field.binding.span)?;
                    if long == "help" {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`--help` is reserved by Argx",
                        ));
                    }
                    if has_version && long == "version" {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`--version` is reserved when command version metadata is present",
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
                    if has_version && *short == b'V' {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`-V` is reserved when command version metadata is present",
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

/// Validates declaration-local relationship references before code generation.
///
/// References into flattened `Args` are resolved later against the composed static tables because
/// the parent proc macro cannot inspect an independently expanded child declaration.
fn validate_constraints(fields: &[Field]) -> syn::Result<()> {
    let has_flatten = fields.iter().any(Field::is_flatten);

    for field in fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        let source = field.binding.name.as_str();

        for (kind, targets) in
            [("requires", &argument.requires), ("conflicts", &argument.conflicts)]
        {
            let mut seen = Vec::<&str>::new();
            for target in targets {
                if target.is_empty() {
                    return Err(syn::Error::new(
                        field.binding.span,
                        format!("`{kind}` must name a Rust argument field"),
                    ));
                }
                if target == source {
                    return Err(syn::Error::new(
                        field.binding.span,
                        format!("`{kind}` cannot reference its own field `{source}`"),
                    ));
                }
                if seen.contains(&target.as_str()) {
                    return Err(syn::Error::new(
                        field.binding.span,
                        format!("duplicate `{kind}` reference `{target}`"),
                    ));
                }
                seen.push(target.as_str());

                if argument.requires.contains(target) && argument.conflicts.contains(target) {
                    return Err(syn::Error::new(
                        field.binding.span,
                        format!(
                            "argument `{source}` cannot both require and conflict with `{target}`"
                        ),
                    ));
                }

                match fields.iter().find(|candidate| candidate.binding.name == *target) {
                    Some(candidate) if candidate.argument().is_some() => {}
                    Some(_) => {
                        return Err(syn::Error::new(
                            field.binding.span,
                            format!("`{kind}` target `{target}` is not an argument field"),
                        ));
                    }
                    None if has_flatten => {}
                    None => {
                        return Err(syn::Error::new(
                            field.binding.span,
                            format!("`{kind}` names no argument field `{target}` in this command"),
                        ));
                    }
                }
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

/// Validates an explicit environment variable name without deferring invalid keys to runtime.
fn validate_env_name(name: &str, span: Span) -> syn::Result<()> {
    if name.is_empty() || name.contains('=') || name.contains('\0') {
        return Err(syn::Error::new(
            span,
            "environment variable name must be non-empty and cannot contain `=` or NUL",
        ));
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

#[cfg(test)]
mod tests {
    use syn::{DeriveInput, parse_quote};

    use super::{ArgumentKind, Command, FieldSemantics, Shape, Subcommand, ValueConversion};

    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers construct owned syntax trees solely for one validation"
    )]
    fn command_error(input: DeriveInput, root: bool) -> String {
        Command::from_input(&input, root)
            .err()
            .expect("command model should be rejected")
            .to_string()
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers construct owned syntax trees solely for one validation"
    )]
    fn subcommand_error(input: DeriveInput) -> String {
        Subcommand::from_input(&input)
            .err()
            .expect("subcommand model should be rejected")
            .to_string()
    }

    #[test]
    fn command_model_separates_cli_semantics_from_rust_binding() {
        let input: DeriveInput = parse_quote! {
            /// Example command.
            #[argx(name = "example")]
            struct Cli {
                /// Enable verbose output.
                #[argx(short, long, global)]
                verbose: bool,
                #[argx(long, env = "ARGX_OUTPUT", default = std::path::PathBuf::from("out"))]
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
        assert_eq!(argument.diagnostic, "--verbose");
        assert!(argument.global);
        assert_eq!(argument.shape, Shape::Bool);
        assert!(verbose.binding.value.is_none());

        let output = &command.fields[1];
        let Some(argument) = output.argument() else {
            panic!("output should be an argument");
        };
        assert!(matches!(&argument.kind, ArgumentKind::Flag { .. }));
        assert_eq!(argument.diagnostic, "--output");
        assert_eq!(argument.shape, Shape::Optional);
        assert_eq!(argument.env.as_deref(), Some("ARGX_OUTPUT"));
        assert!(argument.has_default);
        assert_eq!(output.value_binding().conversion, ValueConversion::Os);
        assert!(output.binding.default.is_some());

        assert!(matches!(&command.fields[2].semantics, FieldSemantics::Flatten));
        assert!(command.fields[2].binding.value.is_none());
        assert!(matches!(&command.fields[3].semantics, FieldSemantics::Subcommand));
        assert!(command.fields[3].binding.value.is_none());
    }

    #[test]
    fn composed_fields_reject_lifetime_and_const_dependencies() {
        let lifetime_input: DeriveInput = parse_quote! {
            struct Cli<'a> {
                #[argx(flatten)]
                shared: Shared<'a>,
            }
        };
        let error = Command::from_input(&lifetime_input, true)
            .err()
            .expect("flattened type must not depend on the containing lifetime");
        assert!(error.to_string().contains("`flatten` cannot depend"));

        let const_input: DeriveInput = parse_quote! {
            struct Cli<const N: usize> {
                #[argx(flatten)]
                shared: Shared<N>,
            }
        };
        let error = Command::from_input(&const_input, true)
            .err()
            .expect("flattened type must not depend on the containing const parameter");
        assert!(error.to_string().contains("`flatten` cannot depend"));

        let subcommand_lifetime_input: DeriveInput = parse_quote! {
            enum Commands<'a> {
                Run(Shared<'a>),
            }
        };
        let error = Subcommand::from_input(&subcommand_lifetime_input)
            .err()
            .expect("payload must not depend on the containing lifetime");
        assert!(error.to_string().contains("subcommand payload cannot depend"));

        let subcommand_const_input: DeriveInput = parse_quote! {
            enum Commands<const N: usize> {
                Run(Shared<N>),
            }
        };
        let error = Subcommand::from_input(&subcommand_const_input)
            .err()
            .expect("payload must not depend on the containing const parameter");
        assert!(error.to_string().contains("subcommand payload cannot depend"));
    }

    #[test]
    fn command_and_subcommand_names_reject_ambiguous_token_spellings() {
        let empty_command: DeriveInput = parse_quote! {
            #[argx(name = "")]
            struct Cli;
        };
        assert_eq!(
            Command::from_input(&empty_command, true)
                .err()
                .expect("empty command name must fail")
                .to_string(),
            "command name cannot be empty",
        );

        let whitespace_subcommand: DeriveInput = parse_quote! {
            enum Commands {
                #[argx(name = "bad name")]
                Run,
            }
        };
        let error = Subcommand::from_input(&whitespace_subcommand)
            .err()
            .expect("whitespace in a subcommand name must fail");
        assert!(error.to_string().contains("subcommand name must be non-empty"));

        let equals_alias: DeriveInput = parse_quote! {
            enum Commands {
                #[argx(alias = "run=now")]
                Run,
            }
        };
        let error = Subcommand::from_input(&equals_alias)
            .err()
            .expect("equals signs in subcommand aliases must fail");
        assert!(error.to_string().contains("subcommand name must be non-empty"));
    }

    #[test]
    fn command_declarations_reject_unsupported_shapes_and_metadata() {
        let error = command_error(
            parse_quote!(
                enum Cli {
                    Run,
                }
            ),
            true,
        );
        assert_eq!(error, "Parser can only be derived for structs");

        let error = command_error(
            parse_quote!(
                struct Cli(String);
            ),
            true,
        );
        assert_eq!(error, "Parser and Args do not support tuple structs; use named fields");

        let error = command_error(
            parse_quote! {
                #[argx(alias = "tool")]
                struct Cli;
            },
            true,
        );
        assert_eq!(error, "command aliases are only valid on Subcommand variants");

        let error = command_error(
            parse_quote! {
                #[argx(version = "1.0")]
                struct Shared;
            },
            false,
        );
        assert_eq!(
            error,
            "version metadata is only valid on Parser declarations and Subcommand variants",
        );
    }

    #[test]
    fn subcommand_declarations_reject_invalid_variants_and_payloads() {
        let error = subcommand_error(parse_quote!(
            struct Commands;
        ));
        assert_eq!(error, "Subcommand can only be derived for enums");

        let error = subcommand_error(parse_quote!(
            enum Commands {}
        ));
        assert_eq!(error, "Subcommand requires at least one variant");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                #[argx(name = "same")]
                First,
                #[argx(name = "same")]
                Second,
            }
        });
        assert_eq!(error, "duplicate subcommand `same`");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                #[argx(alias = "run")]
                Run,
            }
        });
        assert_eq!(error, "duplicate subcommand spelling `run`");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run(Option<Shared>),
            }
        });
        assert_eq!(error, "subcommand payload must be one direct Args type");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run(Shared, Other),
            }
        });
        assert_eq!(error, "subcommand tuple variants must contain exactly one Args payload");

        let error = subcommand_error(parse_quote! {
            enum Commands {
                Run { shared: Shared },
            }
        });
        assert_eq!(
            error,
            "subcommand variants support only unit variants or one unnamed Args payload",
        );
    }

    #[test]
    fn composed_fields_reject_incompatible_roles_attributes_and_wrappers() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, subcommand)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` and `subcommand` cannot be combined");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, requires = "value")]
                    shared: Shared,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` and `conflicts` are only valid on argument fields");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand, long)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`subcommand` cannot be combined with flag, value, or help attributes");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    command: Option<Commands>,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "`subcommand` does not support `Option<T>`; hold the Subcommand enum directly",
        );

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    command: Vec<Commands>,
                }
            },
            true,
        );
        assert_eq!(error, "`subcommand` does not support collection wrappers");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten, long)]
                    shared: Shared,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` cannot be combined with flag, value, or help attributes");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten)]
                    shared: Option<Shared>,
                }
            },
            true,
        );
        assert_eq!(error, "`flatten` does not support `Option<T>`; hold the Args struct directly");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(flatten)]
                    shared: Vec<Shared>,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "`flatten` does not support collection wrappers; hold one Args struct directly",
        );
    }

    #[test]
    fn argument_fields_reject_incompatible_flag_and_value_policies() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(alias = "other")]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`alias` and `aliases` are only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(allow_hyphen_values)]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`allow_hyphen_values` is only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(global)]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(error, "`global` is only valid on named flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, allow_negative_numbers)]
                    verbose: bool,
                }
            },
            true,
        );
        assert_eq!(error, "value policies are not valid on bool fields");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, env = "TOOL_VALUE")]
                    value: Vec<String>,
                }
            },
            true,
        );
        assert_eq!(error, "`env` is only supported on scalar value-taking flags");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, default = true)]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`default` is only supported on scalar value-taking flags");
    }

    #[test]
    fn command_wide_validation_rejects_reserved_duplicate_and_ambiguous_layouts() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "help")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`--help` is reserved by Argx");

        let error = command_error(
            parse_quote! {
                #[argx(version = "1.0")]
                struct Cli {
                    #[argx(short = 'V')]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`-V` is reserved when command version metadata is present");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "same")]
                    first: bool,
                    #[argx(long = "same")]
                    second: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate long flag `--same`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(short = 'x')]
                    first: bool,
                    #[argx(short = 'x')]
                    second: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate short flag `-x`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    optional: Option<String>,
                    required: String,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "required positional arguments cannot follow optional positional arguments"
        );

        let error = command_error(
            parse_quote! {
                struct Cli {
                    values: Vec<String>,
                    later: Option<String>,
                }
            },
            true,
        );
        assert_eq!(error, "variadic positional argument must be the last positional argument");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(subcommand)]
                    first: Commands,
                    #[argx(subcommand)]
                    second: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "a command can contain only one `subcommand` field");
    }

    #[test]
    fn constraint_validation_rejects_invalid_local_relationships() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` must name a Rust argument field");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "value")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` cannot reference its own field `value`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "token", requires = "token")]
                    value: bool,
                    #[argx(long)]
                    token: bool,
                }
            },
            true,
        );
        assert_eq!(error, "duplicate `requires` reference `token`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "token", conflicts = "token")]
                    value: bool,
                    #[argx(long)]
                    token: bool,
                }
            },
            true,
        );
        assert_eq!(error, "argument `value` cannot both require and conflict with `token`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, requires = "command")]
                    value: bool,
                    #[argx(subcommand)]
                    command: Commands,
                }
            },
            true,
        );
        assert_eq!(error, "`requires` target `command` is not an argument field");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, conflicts = "missing")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "`conflicts` names no argument field `missing` in this command");
    }

    #[test]
    fn spelling_and_environment_validation_rejects_invalid_values() {
        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(short = '=')]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(error, "short flag must be one visible ASCII character other than `-` or `=`");

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long = "bad name")]
                    value: bool,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls",
        );

        let error = command_error(
            parse_quote! {
                struct Cli {
                    #[argx(long, env = "BAD=NAME")]
                    value: String,
                }
            },
            true,
        );
        assert_eq!(
            error,
            "environment variable name must be non-empty and cannot contain `=` or NUL"
        );
    }
}
