//! Normalization and validation for `Parser` and `Args` declarations.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields, GenericParam, Type, visit::Visit as _};

use super::{
    Argument, ArgumentKind, Command, CommandBinding, CommandSemantics, Field, FieldBinding,
    FieldSemantics, GenericName, GenericUse, HelpSection, Shape, ValueBinding, ValueConversion,
    ident_name,
    shape::{peel_option, peel_vec, rendered_path, validate_value_shape},
};
use crate::{attrs, case};

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
