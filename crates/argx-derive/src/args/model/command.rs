//! Normalization and validation for `Parser` and `Args` declarations.

use proc_macro2::Span;
use quote::ToTokens as _;
use syn::{Data, DeriveInput, Fields, Type};

use super::{
    Argument, ArgumentKind, Command, CommandBinding, CommandSemantics, Field, FieldBinding,
    FieldSemantics, GenericName, GenericUse, Shape, ValueBinding, ValueConversion, ValueSchema,
    shape::{peel_option, peel_vec, rendered_path, validate_value_shape},
};
use crate::{args::attrs, support};

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
        let mut attributes = attrs::command(&input.attrs)?;
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
        if !root
            && attributes.schema
            && !fields.iter().any(|field| matches!(field.semantics, FieldSemantics::Subcommand))
        {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "`#[argx(schema)]` on Args requires a `#[argx(subcommand)]` field; executable leaves use `#[argx(handler = ...)]`",
            ));
        }
        // Validate every invariant the current macro expansion can see. Cross-flatten invariants
        // are emitted later as const assertions over the composed child tables.
        let has_version = attributes.version.is_some() || attributes.long_version.is_some();
        validate_fields(&fields, has_version, attributes.schema)?;
        validate_constraints(&fields)?;
        validate_relationship_groups(&fields, &attributes.one_of, "one_of", input.ident.span())?;
        validate_relationship_groups(&fields, &attributes.any_of, "any_of", input.ident.span())?;
        validate_composed_generics(&fields, &input.generics)?;
        validate_value_enum_generics(&fields, &input.generics)?;

        // Only after validation do we derive human-facing metadata. This keeps inferred names and
        // doc-derived help in the same semantic representation as explicit attribute overrides.
        let rust_name = support::ident_name(&input.ident);
        let name = attributes.name.take().unwrap_or_else(|| support::to_kebab(&rust_name));
        if name.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "command name cannot be empty"));
        }
        let semantics =
            CommandSemantics::from_attrs(name, attributes, attrs::doc_help(&input.attrs));

        Ok(Self {
            binding: CommandBinding {
                ident: input.ident.clone(),
                visibility: input.vis.clone(),
                generics: input.generics.clone(),
                fingerprint: input.to_token_stream().to_string(),
                root,
                unit,
            },
            semantics,
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
        let name = support::ident_name(&ident);
        let binding = FieldBinding {
            span: ident.span(),
            ident,
            ty: field.ty.clone(),
            name,
            value: None,
            default: None,
        };

        validate_structural_role(&attributes, binding.span)?;
        if attributes.subcommand {
            return Self::subcommand_field(binding, &attributes);
        }
        if attributes.flatten {
            return Self::flatten_field(field, binding, &attributes);
        }
        Self::argument_field(field, binding, attributes)
    }

    /// Normalizes one field that selects a derived `Subcommand` declaration.
    fn subcommand_field(
        binding: FieldBinding,
        attributes: &attrs::FieldAttrs,
    ) -> syn::Result<Self> {
        validate_structural_metadata(
            attributes,
            binding.span,
            "`subcommand` cannot be combined with flag, value, or help attributes",
        )?;
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
        Ok(Self { binding, semantics: FieldSemantics::Subcommand, help_heading: None })
    }

    /// Normalizes one field that composes a derived `Args` declaration inline.
    fn flatten_field(
        field: &syn::Field,
        binding: FieldBinding,
        attributes: &attrs::FieldAttrs,
    ) -> syn::Result<Self> {
        validate_structural_metadata(
            attributes,
            binding.span,
            "`flatten` cannot be combined with flag, value, or help attributes",
        )?;
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

        Ok(Self {
            binding,
            semantics: FieldSemantics::Flatten,
            help_heading: attrs::doc_heading(&field.attrs),
        })
    }

    /// Normalizes one ordinary flag or positional field after structural roles are excluded.
    fn argument_field(
        field: &syn::Field,
        mut binding: FieldBinding,
        mut attributes: attrs::FieldAttrs,
    ) -> syn::Result<Self> {
        // Preserve validation order: shape errors precede spelling errors, which precede value
        // policy errors. These diagnostics are part of the derive's user-facing contract.
        let shape = Shape::from_type(&binding.ty);
        validate_value_shape(&binding.ty, shape, binding.span)?;
        let kind = argument_kind(&binding, &mut attributes)?;
        validate_count(&attributes, &binding.ty, &kind, binding.span)?;
        validate_delimited(&attributes, shape, binding.span)?;
        validate_argument_value_policies(&attributes, &kind, shape, binding.span)?;
        validate_argument_default(&attributes, &kind, shape, binding.span)?;

        let diagnostic = argument_diagnostic(&binding, &kind);
        let has_default = attributes.default.is_some();
        let default_value = attributes.help_default.clone().or_else(|| {
            attributes
                .default
                .as_ref()
                .and_then(|expression| support::default_help(expression, attributes.value_enum))
        });
        let switch = matches!(&kind, ArgumentKind::Flag { .. }) && shape == Shape::Bool;
        let count = attributes.count;
        if attributes.value_enum && switch {
            return Err(syn::Error::new(
                binding.span,
                "`value_enum` is only valid on value-taking arguments",
            ));
        }
        if !switch && !count {
            binding.value = Some(value_binding(&binding.ty, shape));
        }
        binding.default = attributes.default;

        let docs = attrs::doc_help(&field.attrs);
        let help = attributes.help.clone().or(docs.summary);
        let long_help = attributes.help.or(docs.description).or_else(|| help.clone());
        Ok(Self {
            binding,
            semantics: FieldSemantics::Argument(Argument {
                help,
                long_help,
                kind,
                diagnostic,
                global: attributes.global,
                shape,
                has_default,
                default_value,
                requires: attributes.requires.into_iter().map(|value| value.value()).collect(),
                conflicts: attributes.conflicts.into_iter().map(|value| value.value()).collect(),
                allow_hyphen_values: attributes.allow_hyphen_values,
                allow_negative_numbers: attributes.allow_negative_numbers,
                value_enum: attributes.value_enum,
                count,
                delimited: attributes.delimited,
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

    /// Reports whether this field binds the number of flag occurrences.
    pub(crate) fn is_count(&self) -> bool {
        self.argument().is_some_and(|argument| argument.count)
    }

    /// Reports whether this argument consumes a value from argv.
    pub(crate) fn takes_value(&self) -> bool {
        self.argument().is_some_and(|argument| {
            matches!(argument.kind, ArgumentKind::Positional)
                || (!self.is_switch() && !argument.count)
        })
    }

    /// Reports whether this argument may occur more than once.
    pub(crate) fn is_repeatable(&self) -> bool {
        self.argument().is_some_and(|argument| argument.shape == Shape::Many || argument.count)
    }

    /// Returns normalized typed-value binding information.
    pub(crate) const fn value_binding(&self) -> &ValueBinding {
        self.binding.value.as_ref().expect("composed fields and switches do not bind typed values")
    }
}

/// Rejects contradictory structural roles and relationship metadata before role-specific checks.
fn validate_structural_role(attributes: &attrs::FieldAttrs, span: Span) -> syn::Result<()> {
    if attributes.flatten && attributes.subcommand {
        return Err(syn::Error::new(span, "`flatten` and `subcommand` cannot be combined"));
    }
    if (attributes.flatten || attributes.subcommand)
        && (!attributes.requires.is_empty() || !attributes.conflicts.is_empty())
    {
        return Err(syn::Error::new(
            span,
            "`requires` and `conflicts` are only valid on argument fields",
        ));
    }
    Ok(())
}

/// Rejects ordinary argument metadata on a structural `flatten` or `subcommand` field.
fn validate_structural_metadata(
    attributes: &attrs::FieldAttrs,
    span: Span,
    message: &str,
) -> syn::Result<()> {
    let has_argument_metadata = [
        attributes.long.is_some(),
        attributes.short.is_some(),
        !attributes.aliases.is_empty(),
        attributes.global,
        attributes.count,
        attributes.delimited,
        attributes.default.is_some(),
        attributes.allow_hyphen_values,
        attributes.allow_negative_numbers,
        attributes.value_enum,
        attributes.help.is_some(),
        attributes.help_default.is_some(),
    ]
    .into_iter()
    .any(|present| present);
    if has_argument_metadata {
        return Err(syn::Error::new(span, message));
    }
    Ok(())
}

/// Classifies one ordinary field as a named flag or positional and normalizes its spellings.
fn argument_kind(
    binding: &FieldBinding,
    attributes: &mut attrs::FieldAttrs,
) -> syn::Result<ArgumentKind> {
    let named = attributes.long.is_some() || attributes.short.is_some();
    if !named && !attributes.aliases.is_empty() {
        return Err(syn::Error::new(
            binding.span,
            "`alias` and `aliases` are only valid on named flags",
        ));
    }
    if !named {
        return Ok(ArgumentKind::Positional);
    }

    let longs = attributes
        .long
        .take()
        .map(|long| match long {
            attrs::Inferred::Infer => support::to_kebab(&binding.name),
            attrs::Inferred::Explicit(value) => value,
        })
        .into_iter()
        .collect();
    let shorts = attributes
        .short
        .take()
        .map(|short| match short {
            attrs::Inferred::Infer => infer_short(&binding.name, binding.span),
            attrs::Inferred::Explicit(value) => validate_short(value, binding.span),
        })
        .transpose()?
        .into_iter()
        .collect();
    let aliases = std::mem::take(&mut attributes.aliases);
    Ok(ArgumentKind::Flag { longs, aliases, shorts })
}

/// Validates counted flags before ordinary value policy checks.
fn validate_count(
    attributes: &attrs::FieldAttrs,
    ty: &Type,
    kind: &ArgumentKind,
    span: Span,
) -> syn::Result<()> {
    if !attributes.count {
        return Ok(());
    }
    if !matches!(kind, ArgumentKind::Flag { .. }) {
        return Err(syn::Error::new(span, "`count` is only valid on named flags"));
    }
    if !matches!(
        rendered_path(ty).as_str(),
        "u8" | "std::primitive::u8"
            | "::std::primitive::u8"
            | "core::primitive::u8"
            | "::core::primitive::u8"
    ) {
        return Err(syn::Error::new(span, "`count` requires a `u8` field"));
    }
    if attributes.value_enum {
        return Err(syn::Error::new(span, "`value_enum` is not valid on counted flags"));
    }
    Ok(())
}

/// Validates comma-delimited parsing before ordinary value policy checks.
fn validate_delimited(attributes: &attrs::FieldAttrs, shape: Shape, span: Span) -> syn::Result<()> {
    if attributes.delimited && shape != Shape::Many {
        return Err(syn::Error::new(span, "`delimited` requires a collection field"));
    }
    Ok(())
}

/// Validates value-consumption policies whose legality depends on argument kind and shape.
fn validate_argument_value_policies(
    attributes: &attrs::FieldAttrs,
    kind: &ArgumentKind,
    shape: Shape,
    span: Span,
) -> syn::Result<()> {
    if attributes.allow_hyphen_values && matches!(kind, ArgumentKind::Positional) {
        return Err(syn::Error::new(span, "`allow_hyphen_values` is only valid on named flags"));
    }
    if attributes.global && matches!(kind, ArgumentKind::Positional) {
        return Err(syn::Error::new(span, "`global` is only valid on named flags"));
    }
    if (attributes.allow_hyphen_values || attributes.allow_negative_numbers) && attributes.count {
        return Err(syn::Error::new(span, "value policies are not valid on counted flags"));
    }
    if (attributes.allow_hyphen_values || attributes.allow_negative_numbers) && shape == Shape::Bool
    {
        return Err(syn::Error::new(span, "value policies are not valid on bool fields"));
    }
    Ok(())
}

/// Validates that a typed default is attached only to a scalar value-taking flag.
fn validate_argument_default(
    attributes: &attrs::FieldAttrs,
    kind: &ArgumentKind,
    shape: Shape,
    span: Span,
) -> syn::Result<()> {
    if attributes.default.is_some()
        && (!matches!(kind, ArgumentKind::Flag { .. })
            || (!attributes.count && matches!(shape, Shape::Bool | Shape::Many)))
    {
        return Err(syn::Error::new(
            span,
            "`default` is only supported on scalar value-taking flags",
        ));
    }
    Ok(())
}

/// Chooses the canonical spelling used by parser diagnostics for one normalized argument.
fn argument_diagnostic(binding: &FieldBinding, kind: &ArgumentKind) -> String {
    match kind {
        ArgumentKind::Flag { longs, shorts, .. } => longs.first().map_or_else(
            || {
                shorts.first().map_or_else(
                    || binding.name.clone(),
                    |short| format!("-{}", char::from(*short)),
                )
            },
            |long| format!("--{long}"),
        ),
        ArgumentKind::Positional => {
            format!("<{}>", binding.name.replace('-', "_").to_ascii_uppercase())
        }
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
    let schema = if ["DateTime<", "chrono::DateTime<", "::chrono::DateTime<"]
        .iter()
        .any(|prefix| rendered.starts_with(prefix))
    {
        ValueSchema::DateTime
    } else {
        match rendered.as_str() {
            "bool" => ValueSchema::Boolean,
            "i8" => ValueSchema::I8,
            "i16" => ValueSchema::I16,
            "i32" => ValueSchema::I32,
            "i64" => ValueSchema::I64,
            "i128" => ValueSchema::I128,
            "isize" => ValueSchema::Isize,
            "u8" => ValueSchema::U8,
            "u16" => ValueSchema::U16,
            "u32" => ValueSchema::U32,
            "u64" => ValueSchema::U64,
            "u128" => ValueSchema::U128,
            "usize" => ValueSchema::Usize,
            "f32" | "f64" => ValueSchema::Number,
            "NaiveDate" | "chrono::NaiveDate" | "::chrono::NaiveDate" => ValueSchema::Date,
            "Uuid" | "uuid::Uuid" | "::uuid::Uuid" => ValueSchema::Uuid,
            "Url" | "url::Url" | "::url::Url" => ValueSchema::Url,
            _ => ValueSchema::Lexical,
        }
    };
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
        schema,
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
fn validate_fields(fields: &[Field], has_version: bool, schema_enabled: bool) -> syn::Result<()> {
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
                    if schema_enabled && long == "schema" {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`--schema` is reserved when schema discovery is enabled",
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
                    if schema_enabled && *short == b'S' {
                        return Err(syn::Error::new(
                            field.binding.span,
                            "`-S` is reserved when schema discovery is enabled",
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

/// Validates declaration-local grouped relationship references before code generation.
///
/// References into flattened `Args` are resolved later against the composed static tables.
fn validate_relationship_groups(
    fields: &[Field],
    groups: &[Vec<String>],
    relationship: &str,
    span: Span,
) -> syn::Result<()> {
    let has_flatten = fields.iter().any(Field::is_flatten);

    for group in groups {
        let mut seen = Vec::new();
        for target in group {
            if target.is_empty() {
                return Err(syn::Error::new(
                    span,
                    format!("`{relationship}` must name Rust argument fields"),
                ));
            }
            if seen.contains(target) {
                return Err(syn::Error::new(
                    span,
                    format!("duplicate `{relationship}` argument field `{target}`"),
                ));
            }
            seen.push(target.clone());

            match fields.iter().find(|field| field.binding.name == *target) {
                Some(field) if field.argument().is_some() => {}
                Some(_) => {
                    return Err(syn::Error::new(
                        span,
                        format!("`{relationship}` target `{target}` is not an argument field"),
                    ));
                }
                None if has_flatten => {}
                None => {
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "`{relationship}` names no argument field `{target}` in this command"
                        ),
                    ));
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
    let params = GenericName::collect(generics);
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
        if GenericUse::finds(&params, ty) {
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

/// Rejects finite-value metadata that depends on the containing command's generic parameters.
///
/// Accepted values live in static command metadata, so the value type must be nameable without a
/// command monomorphization. Concrete generic types remain valid; only references to the containing
/// declaration's own generic parameters are rejected.
fn validate_value_enum_generics(fields: &[Field], generics: &syn::Generics) -> syn::Result<()> {
    let params = GenericName::collect(generics);
    if params.is_empty() {
        return Ok(());
    }

    for field in fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        if !argument.value_enum {
            continue;
        }
        let ty = &field.value_binding().ty;
        if GenericUse::finds(&params, ty) {
            return Err(syn::Error::new_spanned(
                ty,
                "`value_enum` cannot depend on the containing struct's generic parameters; use a concrete ValueEnum type",
            ));
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
