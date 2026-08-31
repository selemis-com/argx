//! Parsing for Argx container and field attributes.
//!
//! This module answers only syntactic questions: which `#[argx(...)]` keys were written and what
//! literal or expression values they contain. Cross-field meaning is validated later in `model`,
//! after Rust documentation and inferred spellings have been normalized. Keeping parsing and
//! semantic validation separate avoids making code generation depend on raw attribute syntax.

use syn::{Attribute, Expr, Lit, LitChar, LitStr, Meta, Token, braced, bracketed, parenthesized};

use crate::args::metadata::{MetadataEntry, MetadataValue};

/// Structured help extracted from Rust doc comments on a command declaration.
///
/// Rustdoc attributes are normalized into one summary, one pre-heading description, and ordered
/// level-one sections. Lower-level Markdown headings remain part of the surrounding section body.
pub(crate) struct DocHelp {
    /// First prose paragraph, collapsed to one line for command listings.
    pub summary: Option<String>,
    /// Full prose before the first level-one heading.
    pub description: Option<String>,
    /// User-authored level-one sections in declaration order.
    pub sections: Vec<DocSection>,
}

/// One level-one help section extracted from Rust doc comments.
pub(crate) struct DocSection {
    /// Section heading without the Markdown marker.
    pub heading: String,
    /// Section body with paragraph and code-block line structure preserved.
    pub body: String,
}

/// An attribute whose bare form asks Argx to infer a value from the Rust identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inferred<T> {
    /// The attribute was present without an explicit value.
    Infer,
    /// The attribute supplied an explicit value.
    Explicit(T),
}

/// Attributes accepted on a command struct.
#[derive(Default)]
pub(crate) struct CommandAttrs {
    /// Explicit command-line name.
    pub name: Option<String>,
    /// Explicit one-line command description.
    pub about: Option<String>,
    /// Short version text expression.
    pub version: Option<Expr>,
    /// Long version text expression.
    pub long_version: Option<Expr>,
    /// Additional hidden spellings accepted for a selectable subcommand.
    pub aliases: Vec<String>,
    /// Application-defined semantic metadata exposed to machine-readable consumers.
    pub metadata: Vec<MetadataEntry>,
    /// Whether this command declaration participates in schema discovery.
    pub schema: bool,
}

/// Attributes accepted on a command field.
#[derive(Default)]
pub(crate) struct FieldAttrs {
    /// Whether this field contributes another `Args` declaration inline.
    pub flatten: bool,
    /// Whether this field selects one command from a derived subcommand enum.
    pub subcommand: bool,
    /// Whether this named argument remains in scope for descendant commands.
    pub global: bool,
    /// Whether this value-less flag binds its occurrence count.
    pub count: bool,
    /// Whether repeated collection values are split on commas.
    pub delimited: bool,
    /// Typed Rust expression used when the argument is absent.
    pub default: Option<Expr>,
    /// Long flag spelling, or an instruction to infer it.
    pub long: Option<Inferred<String>>,
    /// Short flag spelling, or an instruction to infer it.
    pub short: Option<Inferred<char>>,
    /// Additional hidden long spellings accepted for this flag.
    pub aliases: Vec<String>,
    /// Arguments that must be satisfied when this argument is supplied.
    pub requires: Vec<LitStr>,
    /// Arguments that cannot be supplied together with this argument.
    pub conflicts: Vec<LitStr>,
    /// Whether detached values may be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
    /// Whether this value-bearing field uses a finite `ValueEnum` vocabulary.
    pub value_enum: bool,
    /// Explicit one-line argument description.
    pub help: Option<String>,
    /// Internal static default spelling forwarded by the Config derive.
    pub help_default: Option<String>,
}

/// Parses every `#[argx(...)]` attribute on a command declaration.
pub(crate) fn command(attributes: &[Attribute]) -> syn::Result<CommandAttrs> {
    command_like(attributes, "command")
}

/// Parses every `#[argx(...)]` attribute on a subcommand variant.
pub(crate) fn variant(attributes: &[Attribute]) -> syn::Result<CommandAttrs> {
    command_like(attributes, "subcommand variant")
}

/// Parses metadata shared by root commands and selectable subcommands.
fn command_like(attributes: &[Attribute], context: &str) -> syn::Result<CommandAttrs> {
    let mut parsed = CommandAttrs::default();
    let mut metadata_seen = false;
    for attribute in attributes.iter().filter(|attribute| attribute.path().is_ident("argx")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if parsed.name.is_some() {
                    return Err(meta.error("duplicate `name` attribute"));
                }
                let value = meta.value()?.parse::<LitStr>()?;
                parsed.name = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("about") {
                if parsed.about.is_some() {
                    return Err(meta.error("duplicate `about` attribute"));
                }
                parsed.about = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else if meta.path.is_ident("version") {
                if parsed.version.is_some() {
                    return Err(meta.error("duplicate `version` attribute"));
                }
                parsed.version = Some(meta.value()?.parse::<Expr>()?);
                Ok(())
            } else if meta.path.is_ident("long_version") {
                if parsed.long_version.is_some() {
                    return Err(meta.error("duplicate `long_version` attribute"));
                }
                parsed.long_version = Some(meta.value()?.parse::<Expr>()?);
                Ok(())
            } else if meta.path.is_ident("alias") {
                parsed.aliases.push(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else if meta.path.is_ident("aliases") {
                parsed.aliases.extend(string_array(&meta)?);
                Ok(())
            } else if meta.path.is_ident("metadata") {
                if metadata_seen {
                    return Err(meta.error("duplicate `metadata` attribute"));
                }
                metadata_seen = true;
                let content;
                parenthesized!(content in meta.input);
                let object;
                braced!(object in content);
                parsed.metadata = metadata_entries(&object)?;
                if !content.is_empty() {
                    return Err(content.error("metadata expects exactly one object"));
                }
                Ok(())
            } else if meta.path.is_ident("schema") {
                if parsed.schema {
                    return Err(meta.error("duplicate `schema` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`schema` takes no value"));
                }
                parsed.schema = true;
                Ok(())
            } else {
                Err(meta.error(format!("unsupported Argx {context} attribute")))
            }
        })?;
    }
    Ok(parsed)
}

/// Parses one JSON-like metadata object while preserving keys exactly as authored.
fn metadata_entries(input: syn::parse::ParseStream<'_>) -> syn::Result<Vec<MetadataEntry>> {
    let mut entries = Vec::new();
    while !input.is_empty() {
        let key = input.parse::<LitStr>()?.value();
        input.parse::<Token![:]>()?;
        let value = metadata_value(input)?;
        if entries.iter().any(|entry: &MetadataEntry| entry.key == key) {
            return Err(input.error(format!("duplicate metadata key `{key}`")));
        }
        entries.push(MetadataEntry { key, value });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(entries)
}

/// Parses one JSON-like metadata object.
fn metadata_object(input: syn::parse::ParseStream<'_>) -> syn::Result<MetadataValue> {
    Ok(MetadataValue::Object(metadata_entries(input)?))
}

/// Parses one JSON-like metadata value.
fn metadata_value(input: syn::parse::ParseStream<'_>) -> syn::Result<MetadataValue> {
    if input.peek(syn::token::Brace) {
        let object;
        braced!(object in input);
        return metadata_object(&object);
    }
    if input.peek(syn::token::Bracket) {
        let array;
        bracketed!(array in input);
        let mut values = Vec::new();
        while !array.is_empty() {
            values.push(metadata_value(&array)?);
            if array.is_empty() {
                break;
            }
            array.parse::<Token![,]>()?;
        }
        return Ok(MetadataValue::Array(values));
    }

    let expression = input.parse::<Expr>()?;
    match expression {
        Expr::Lit(expression) => match expression.lit {
            Lit::Bool(value) => Ok(MetadataValue::Bool(value.value)),
            Lit::Int(value) => value
                .base10_parse::<i64>()
                .map(MetadataValue::Integer)
                .map_err(|_| syn::Error::new_spanned(value, "metadata integer must fit in i64")),
            Lit::Float(value) => {
                let parsed = value
                    .base10_parse::<f64>()
                    .map_err(|_| syn::Error::new_spanned(&value, "invalid metadata float"))?;
                if !parsed.is_finite() {
                    return Err(syn::Error::new_spanned(value, "metadata float must be finite"));
                }
                Ok(MetadataValue::Float(parsed))
            }
            Lit::Str(value) => Ok(MetadataValue::String(value.value())),
            other => Err(syn::Error::new_spanned(
                other,
                "metadata values must be null, booleans, numbers, strings, arrays, or objects",
            )),
        },
        Expr::Path(path) if path.path.is_ident("null") => Ok(MetadataValue::Null),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => match *unary.expr {
            Expr::Lit(expression) => match expression.lit {
                Lit::Int(value) => value
                    .base10_parse::<i64>()
                    .ok()
                    .and_then(i64::checked_neg)
                    .map(MetadataValue::Integer)
                    .ok_or_else(|| {
                        syn::Error::new_spanned(value, "metadata integer must fit in i64")
                    }),
                Lit::Float(value) => {
                    let parsed = value
                        .base10_parse::<f64>()
                        .map_err(|_| syn::Error::new_spanned(&value, "invalid metadata float"))?;
                    let parsed = -parsed;
                    if !parsed.is_finite() {
                        return Err(syn::Error::new_spanned(
                            value,
                            "metadata float must be finite",
                        ));
                    }
                    Ok(MetadataValue::Float(parsed))
                }
                other => Err(syn::Error::new_spanned(
                    other,
                    "metadata values must be null, booleans, numbers, strings, arrays, or objects",
                )),
            },
            other => Err(syn::Error::new_spanned(
                other,
                "metadata values must be null, booleans, numbers, strings, arrays, or objects",
            )),
        },
        other => Err(syn::Error::new_spanned(
            other,
            "metadata values must be null, booleans, numbers, strings, arrays, or objects",
        )),
    }
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
            } else if meta.path.is_ident("subcommand") {
                if parsed.subcommand {
                    return Err(meta.error("duplicate `subcommand` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`subcommand` takes no value"));
                }
                parsed.subcommand = true;
                Ok(())
            } else if meta.path.is_ident("global") {
                if parsed.global {
                    return Err(meta.error("duplicate `global` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`global` takes no value"));
                }
                parsed.global = true;
                Ok(())
            } else if meta.path.is_ident("delimited") {
                if parsed.delimited {
                    return Err(meta.error("duplicate `delimited` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`delimited` takes no value"));
                }
                parsed.delimited = true;
                Ok(())
            } else if meta.path.is_ident("count") {
                if parsed.count {
                    return Err(meta.error("duplicate `count` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`count` takes no value"));
                }
                parsed.count = true;
                Ok(())
            } else if meta.path.is_ident("default") {
                if parsed.default.is_some() {
                    return Err(meta.error("duplicate `default` attribute"));
                }
                parsed.default = Some(meta.value()?.parse::<Expr>()?);
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
            } else if meta.path.is_ident("alias") {
                parsed.aliases.push(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else if meta.path.is_ident("aliases") {
                parsed.aliases.extend(string_array(&meta)?);
                Ok(())
            } else if meta.path.is_ident("requires") {
                parsed.requires.extend(string_or_array(&meta, "requires")?);
                Ok(())
            } else if meta.path.is_ident("conflicts") {
                parsed.conflicts.extend(string_or_array(&meta, "conflicts")?);
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
            } else if meta.path.is_ident("__help_default") {
                if parsed.help_default.is_some() {
                    return Err(meta.error("duplicate internal help default"));
                }
                parsed.help_default = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else if meta.path.is_ident("value_enum") {
                if parsed.value_enum {
                    return Err(meta.error("duplicate `value_enum` attribute"));
                }
                if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`value_enum` takes no value"));
                }
                parsed.value_enum = true;
                Ok(())
            } else if meta.path.is_ident("help") {
                if parsed.help.is_some() {
                    return Err(meta.error("duplicate `help` attribute"));
                }
                parsed.help = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else {
                Err(meta.error("unsupported Argx field attribute"))
            }
        })?;
    }
    Ok(parsed)
}

/// Parses either one string or a non-empty array of strings.
fn string_or_array(
    meta: &syn::meta::ParseNestedMeta<'_>,
    attribute: &str,
) -> syn::Result<Vec<LitStr>> {
    if !meta.input.peek(Token![=]) {
        return Err(meta.error(format!("`{attribute}` expects a string or non-empty string array")));
    }

    match meta.value()?.parse::<Expr>()? {
        Expr::Lit(syn::ExprLit { lit: Lit::Str(value), .. }) => Ok(vec![value]),
        Expr::Array(array) => {
            if array.elems.is_empty() {
                return Err(syn::Error::new_spanned(
                    array,
                    format!("`{attribute}` array must contain at least one target"),
                ));
            }
            array
                .elems
                .into_iter()
                .map(|expression| match expression {
                    Expr::Lit(syn::ExprLit { lit: Lit::Str(value), .. }) => Ok(value),
                    _ => Err(syn::Error::new_spanned(
                        expression,
                        format!("`{attribute}` targets must be string literals"),
                    )),
                })
                .collect()
        }
        expression => Err(syn::Error::new_spanned(
            expression,
            format!("`{attribute}` expects a string or non-empty string array"),
        )),
    }
}

/// Parses a non-empty array of string values from a plural attribute.
fn string_array(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Vec<String>> {
    if !meta.input.peek(Token![=]) {
        return Err(meta.error("plural alias attributes require a string array"));
    }
    let array = meta.value()?.parse::<syn::ExprArray>()?;
    if array.elems.is_empty() {
        return Err(meta.error("plural alias attributes require at least one value"));
    }
    array
        .elems
        .iter()
        .map(|expression| match expression {
            Expr::Lit(syn::ExprLit { lit: Lit::Str(value), .. }) => Ok(value.value()),
            _ => Err(syn::Error::new_spanned(expression, "alias values must be string literals")),
        })
        .collect()
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

/// Returns a leading level-one Markdown heading from Rust doc comments.
///
/// Flattened argument groups are explicit: ordinary field prose remains documentation, while a
/// leading `# Heading` opts the flattened field into a named terminal help section.
pub(crate) fn doc_heading(attributes: &[Attribute]) -> Option<String> {
    let lines = doc_source_lines(attributes);
    trim_blank_lines(&lines).first().and_then(|line| level_one_heading(line)).map(str::to_owned)
}

/// Parses command-level Rust docs into prose plus user-authored level-one help sections.
pub(crate) fn doc_help(attributes: &[Attribute]) -> DocHelp {
    let source_lines = doc_source_lines(attributes);
    let summary = first_paragraph(trim_blank_lines(&source_lines));
    let lines = strip_text_fences(source_lines);
    let first_heading =
        lines.iter().position(|line| level_one_heading(line).is_some()).unwrap_or(lines.len());
    let preamble = trim_blank_lines(&lines[..first_heading]);
    let description = (!preamble.is_empty()).then(|| preamble.join("\n"));

    let mut sections = Vec::new();
    let mut index = first_heading;
    while index < lines.len() {
        let Some(heading) = level_one_heading(&lines[index]) else {
            index += 1;
            continue;
        };
        index += 1;
        let body_start = index;
        while index < lines.len() && level_one_heading(&lines[index]).is_none() {
            index += 1;
        }
        let body = trim_blank_lines(&lines[body_start..index]).join("\n");
        sections.push(DocSection { heading: heading.to_owned(), body });
    }

    DocHelp { summary, description, sections }
}

/// Extracts normalized source lines while retaining fences needed to delimit short prose help.
fn doc_source_lines(attributes: &[Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"))
        .filter_map(|attribute| {
            let Meta::NameValue(meta) = &attribute.meta else {
                return None;
            };
            let Expr::Lit(value) = &meta.value else {
                return None;
            };
            let Lit::Str(value) = &value.lit else {
                return None;
            };
            let line = value.value();
            Some(line.strip_prefix(' ').unwrap_or(&line).trim_end().to_owned())
        })
        .collect()
}

/// Returns a level-one Markdown heading, excluding deeper headings such as `## Details`.
fn level_one_heading(line: &str) -> Option<&str> {
    line.strip_prefix("# ").map(str::trim).filter(|heading| !heading.is_empty())
}

/// Removes only blank boundary lines while preserving body formatting.
fn trim_blank_lines(lines: &[String]) -> &[String] {
    let start = lines.iter().position(|line| !line.trim().is_empty()).unwrap_or(lines.len());
    let end =
        lines.iter().rposition(|line| !line.trim().is_empty()).map_or(start, |index| index + 1);
    &lines[start..end]
}

/// Collapses the first prose paragraph to one line for short command descriptions.
fn first_paragraph(lines: &[String]) -> Option<String> {
    let paragraph = lines
        .iter()
        .take_while(|line| !line.trim().is_empty() && text_fence(line).is_none())
        .collect::<Vec<_>>();
    (!paragraph.is_empty())
        .then(|| paragraph.into_iter().map(|line| line.trim()).collect::<Vec<_>>().join(" "))
}

/// Removes explicit `text` fences while retaining their contents as rendered help text.
fn strip_text_fences(lines: Vec<String>) -> Vec<String> {
    let mut output = Vec::with_capacity(lines.len());
    let mut fence = None;

    for line in lines {
        if let Some(active) = fence {
            if line.trim() == active {
                fence = None;
                continue;
            }
        } else if let Some(opening) = text_fence(&line) {
            if output.last().is_some_and(|line: &String| line.trim().is_empty()) {
                output.pop();
            }
            fence = Some(opening);
            continue;
        }
        output.push(line);
    }

    output
}

/// Returns the closing delimiter for an explicit text Markdown fence.
fn text_fence(line: &str) -> Option<&'static str> {
    match line.trim() {
        "```text" => Some("```"),
        "~~~text" => Some("~~~"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use syn::{DeriveInput, parse_quote};

    use super::*;

    #[test]
    fn command_metadata_preserves_keys_and_nested_values() {
        let input: DeriveInput = parse_quote! {
            #[argx(metadata({
                "readOnly": true,
                "requiredScopes": ["objects:read"],
                "policy": { "owner": "knowledge", "level": 2 },
            }))]
            struct Example;
        };

        let attributes = command(&input.attrs).expect("command attributes should parse");
        assert_eq!(attributes.metadata.len(), 3);
        assert_eq!(attributes.metadata[0].key, "readOnly");
        assert_eq!(attributes.metadata[1].key, "requiredScopes");
        assert_eq!(attributes.metadata[2].key, "policy");
        assert!(matches!(&attributes.metadata[2].value, MetadataValue::Object(_)));
    }

    #[test]
    fn command_metadata_rejects_duplicate_keys() {
        let input: DeriveInput = parse_quote! {
            #[argx(metadata({ "readOnly": true, "readOnly": false }))]
            struct Example;
        };

        let error = match command(&input.attrs) {
            Ok(_) => panic!("duplicate metadata key must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("duplicate metadata key `readOnly`"));
    }

    #[test]
    fn doc_summary_uses_only_the_first_paragraph() {
        let input: DeriveInput = parse_quote! {
            /// First line.
            /// Second line.
            ///
            /// Detailed text is not part of short help.
            struct Example;
        };
        assert_eq!(doc_help(&input.attrs).summary.as_deref(), Some("First line. Second line."));
    }

    #[test]
    fn doc_heading_requires_a_leading_level_one_heading() {
        let prose: DeriveInput = parse_quote! {
            /// Object containing the comment.
            struct Prose;
        };
        assert_eq!(doc_heading(&prose.attrs), None);

        let heading: DeriveInput = parse_quote! {
            /// # Object containing the comment
            struct Heading;
        };
        assert_eq!(doc_heading(&heading.attrs).as_deref(), Some("Object containing the comment"));

        let later_heading: DeriveInput = parse_quote! {
            /// Ordinary field documentation.
            ///
            /// # Details
            struct LaterHeading;
        };
        assert_eq!(doc_heading(&later_heading.attrs), None);
    }

    #[test]
    fn doc_summary_stops_before_a_stripped_text_fence() {
        let input: DeriveInput = parse_quote! {
            /// Some description.
            ///
            /// ~~~text
            ///     __ __ __
            ///    / //_//_/
            /// ~~~
            struct Example;
        };

        let help = doc_help(&input.attrs);
        assert_eq!(help.summary.as_deref(), Some("Some description."));
        assert_eq!(
            help.description.as_deref(),
            Some("Some description.\n    __ __ __\n   / //_//_/")
        );
    }

    #[test]
    fn doc_help_strips_backtick_text_fences() {
        let input: DeriveInput = parse_quote! {
            /// Some description.
            ///
            /// ```text
            ///     __ __ __
            ///    / //_//_/
            /// ```
            struct Example;
        };

        let help = doc_help(&input.attrs);
        assert_eq!(help.summary.as_deref(), Some("Some description."));
        assert_eq!(
            help.description.as_deref(),
            Some("Some description.\n    __ __ __\n   / //_//_/")
        );
    }

    #[test]
    fn doc_help_preserves_non_text_fences() {
        let input: DeriveInput = parse_quote! {
            /// Short summary.
            ///
            /// ```sh
            /// tool run
            /// ```
            ///
            /// ~~~rust
            /// let value = 1;
            /// ~~~
            struct Example;
        };

        let help = doc_help(&input.attrs);
        assert_eq!(
            help.description.as_deref(),
            Some("Short summary.\n\n```sh\ntool run\n```\n\n~~~rust\nlet value = 1;\n~~~")
        );
    }

    #[test]
    fn doc_help_preserves_preamble_and_level_one_sections() {
        let input: DeriveInput = parse_quote! {
            /// Short summary.
            ///
            /// Longer command context.
            ///
            /// # Examples
            ///
            /// ```text
            /// tool run
            /// ```
            ///
            /// ## Detail
            /// Still part of the examples body.
            ///
            /// # Machine-readable usage
            /// Use `tool schema`.
            struct Example;
        };
        let help = doc_help(&input.attrs);
        assert_eq!(help.summary.as_deref(), Some("Short summary."));
        assert_eq!(help.description.as_deref(), Some("Short summary.\n\nLonger command context."));
        assert_eq!(help.sections.len(), 2);
        assert_eq!(help.sections[0].heading, "Examples");
        assert!(help.sections[0].body.contains("tool run"));
        assert!(!help.sections[0].body.contains("```text"));
        assert!(help.sections[0].body.contains("## Detail"));
        assert_eq!(help.sections[1].heading, "Machine-readable usage");
        assert_eq!(help.sections[1].body, "Use `tool schema`.");
    }
}
