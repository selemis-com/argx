//! TOML environment interpolation and escaping.

use std::fmt::Write as _;

use crate::config::{environment::Environment, error::Location};

/// Result of interpolating one TOML source.
#[derive(Debug)]
pub(crate) struct TomlExpansion {
    /// TOML text after placeholder expansion and literal-placeholder unescaping.
    pub(crate) text: String,
    /// Whether at least one environment value was inserted.
    pub(crate) substituted: bool,
}

/// Errors produced while interpolating TOML environment placeholders.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TomlInterpolationError {
    /// A `${...}` placeholder was not terminated.
    #[error(
        "unterminated environment placeholder at line {}, column {}",
        .location.line,
        .location.column,
    )]
    Unterminated {
        /// Source location of the opening `$`.
        location: Location,
    },
    /// A placeholder contained an invalid environment variable name.
    #[error(
        "invalid environment variable name `{name}` in placeholder at line {}, column {}",
        .location.line,
        .location.column,
    )]
    InvalidName {
        /// Invalid variable name.
        name: String,
        /// Source location of the opening `$`.
        location: Location,
    },
    /// A referenced variable was absent from both process environment and dotenv.
    #[error(
        "environment variable `{name}` referenced at line {}, column {} is not set",
        .location.line,
        .location.column,
    )]
    Missing {
        /// Missing variable name.
        name: String,
        /// Source location of the opening `$`.
        location: Location,
    },
    /// A process environment value used for interpolation was not UTF-8.
    #[error(
        "environment variable `{name}` referenced at line {}, column {} is not valid UTF-8",
        .location.line,
        .location.column,
    )]
    NonUtf8 {
        /// Variable whose value was not UTF-8.
        name: String,
        /// Source location of the opening `$`.
        location: Location,
    },
    /// Interpolation was requested inside a TOML literal string.
    #[error(
        "environment variable `{name}` cannot be interpolated inside a TOML literal string at line {}, column {}; use a basic string instead",
        .location.line,
        .location.column,
    )]
    LiteralString {
        /// Variable name that was referenced.
        name: String,
        /// Source location of the opening `$`.
        location: Location,
    },
    /// A placeholder appeared somewhere that can change TOML structure.
    #[error(
        "environment variable `{name}` cannot be interpolated into TOML structure at line {}, column {}; placeholders are only supported inside a top-level basic string value",
        .location.line,
        .location.column,
    )]
    Structural {
        /// Variable name that was referenced.
        name: String,
        /// Source location of the opening `$`.
        location: Location,
    },
}

impl TomlInterpolationError {
    /// Returns the referenced environment variable when one was parsed.
    pub(crate) fn variable(&self) -> Option<&str> {
        match self {
            Self::Unterminated { .. } => None,
            Self::InvalidName { name, .. }
            | Self::Missing { name, .. }
            | Self::NonUtf8 { name, .. }
            | Self::LiteralString { name, .. }
            | Self::Structural { name, .. } => Some(name),
        }
    }

    /// Returns the source location of the placeholder.
    pub(crate) const fn location(&self) -> Location {
        match self {
            Self::Unterminated { location }
            | Self::InvalidName { location, .. }
            | Self::Missing { location, .. }
            | Self::NonUtf8 { location, .. }
            | Self::LiteralString { location, .. }
            | Self::Structural { location, .. } => *location,
        }
    }
}

/// Lexical TOML context relevant to safe placeholder insertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TomlState {
    /// Outside strings and comments.
    Normal,
    /// Inside a single-line basic string (`"..."`).
    Basic,
    /// Inside a single-line literal string (`'...'`).
    Literal,
    /// Inside a multiline basic string (`"""..."""`).
    MultilineBasic,
    /// Inside a multiline literal string (`'''...'''`).
    MultilineLiteral,
    /// Inside a comment until the next newline.
    Comment,
}

/// Expands `${NAME}` placeholders using the environment visible to this layer.
///
/// Interpolation is supported only inside a top-level TOML basic string value,
/// where inserted environment data can be escaped as string content without
/// changing TOML structure. Placeholders in bare values, keys, table names,
/// collection fragments, and literal strings are rejected. `$${NAME}` emits a
/// literal `${NAME}` without reading the environment.
pub(crate) fn expand_toml(
    input: &str,
    environment: &Environment,
) -> Result<TomlExpansion, TomlInterpolationError> {
    let mut output = String::with_capacity(input.len());
    let mut state = TomlState::Normal;
    let mut index = 0;
    let mut substituted = false;
    let mut in_value = false;
    let mut nesting_depth = 0_usize;
    let mut value_has_content = false;
    let mut string_is_top_level_value = false;

    while index < input.len() {
        let remainder = &input[index..];

        if state != TomlState::Comment && remainder.starts_with("$${") {
            output.push_str("${");
            index += 3;
            if state == TomlState::Normal && in_value {
                value_has_content = true;
            }
            continue;
        }

        if state != TomlState::Comment && remainder.starts_with("${") {
            let (name, next) = placeholder(input, index)?;
            let location = Location::from_offset(input, index);

            match state {
                TomlState::Literal | TomlState::MultilineLiteral => {
                    return Err(TomlInterpolationError::LiteralString {
                        name: name.to_owned(),
                        location,
                    });
                }
                TomlState::Basic | TomlState::MultilineBasic => {
                    if !string_is_top_level_value {
                        return Err(TomlInterpolationError::Structural {
                            name: name.to_owned(),
                            location,
                        });
                    }
                    let value = lookup(name, location, environment)?;
                    push_basic_string_value(&mut output, value);
                }
                TomlState::Normal => {
                    return Err(TomlInterpolationError::Structural {
                        name: name.to_owned(),
                        location,
                    });
                }
                TomlState::Comment => {
                    unreachable!("comments are excluded before placeholder parsing")
                }
            }

            substituted = true;
            index = next;
            continue;
        }

        match state {
            TomlState::Normal => {
                if remainder.starts_with("\"\"\"") {
                    output.push_str("\"\"\"");
                    string_is_top_level_value =
                        in_value && nesting_depth == 0 && !value_has_content;
                    if in_value {
                        value_has_content = true;
                    }
                    state = TomlState::MultilineBasic;
                    index += 3;
                } else if remainder.starts_with("'''") {
                    output.push_str("'''");
                    string_is_top_level_value =
                        in_value && nesting_depth == 0 && !value_has_content;
                    if in_value {
                        value_has_content = true;
                    }
                    state = TomlState::MultilineLiteral;
                    index += 3;
                } else {
                    let (character, next) = next_char(input, index);
                    output.push(character);
                    match character {
                        '"' => {
                            string_is_top_level_value =
                                in_value && nesting_depth == 0 && !value_has_content;
                            if in_value {
                                value_has_content = true;
                            }
                            state = TomlState::Basic;
                        }
                        '\'' => {
                            string_is_top_level_value =
                                in_value && nesting_depth == 0 && !value_has_content;
                            if in_value {
                                value_has_content = true;
                            }
                            state = TomlState::Literal;
                        }
                        '#' => state = TomlState::Comment,
                        '=' if !in_value => {
                            in_value = true;
                            nesting_depth = 0;
                            value_has_content = false;
                        }
                        '\n' => {
                            if nesting_depth == 0 {
                                in_value = false;
                                value_has_content = false;
                            }
                        }
                        '[' | '{' if in_value => {
                            nesting_depth = nesting_depth.saturating_add(1);
                            value_has_content = true;
                        }
                        ']' | '}' if in_value && nesting_depth > 0 => {
                            nesting_depth -= 1;
                            value_has_content = true;
                        }
                        character if in_value && !character.is_whitespace() => {
                            value_has_content = true;
                        }
                        _ => {}
                    }
                    index = next;
                }
            }
            TomlState::Comment => {
                let (character, next) = next_char(input, index);
                output.push(character);
                if character == '\n' {
                    state = TomlState::Normal;
                    if nesting_depth == 0 {
                        in_value = false;
                        value_has_content = false;
                    }
                }
                index = next;
            }
            TomlState::Basic => {
                let (character, next) = next_char(input, index);
                output.push(character);
                index = next;
                if character == '\\' && index < input.len() {
                    let (escaped, next) = next_char(input, index);
                    output.push(escaped);
                    index = next;
                } else if character == '"' {
                    state = TomlState::Normal;
                    string_is_top_level_value = false;
                }
            }
            TomlState::Literal => {
                let (character, next) = next_char(input, index);
                output.push(character);
                index = next;
                if character == '\'' {
                    state = TomlState::Normal;
                    string_is_top_level_value = false;
                }
            }
            TomlState::MultilineBasic => {
                if remainder.starts_with("\"\"\"") {
                    output.push_str("\"\"\"");
                    state = TomlState::Normal;
                    string_is_top_level_value = false;
                    index += 3;
                } else {
                    let (character, next) = next_char(input, index);
                    output.push(character);
                    index = next;
                    if character == '\\' && index < input.len() {
                        let (escaped, next) = next_char(input, index);
                        output.push(escaped);
                        index = next;
                    }
                }
            }
            TomlState::MultilineLiteral => {
                if remainder.starts_with("'''") {
                    output.push_str("'''");
                    state = TomlState::Normal;
                    string_is_top_level_value = false;
                    index += 3;
                } else {
                    let (character, next) = next_char(input, index);
                    output.push(character);
                    index = next;
                }
            }
        }
    }

    Ok(TomlExpansion { text: output, substituted })
}

/// Parses and validates one `${NAME}` placeholder.
fn placeholder(input: &str, index: usize) -> Result<(&str, usize), TomlInterpolationError> {
    let name_start = index + 2;
    let Some(relative_end) = input[name_start..].find('}') else {
        return Err(TomlInterpolationError::Unterminated {
            location: Location::from_offset(input, index),
        });
    };
    let name_end = name_start + relative_end;
    let name = &input[name_start..name_end];
    if !valid_name(name) {
        return Err(TomlInterpolationError::InvalidName {
            name: name.to_owned(),
            location: Location::from_offset(input, index),
        });
    }
    Ok((name, name_end + 1))
}

/// Returns whether `name` is a portable environment identifier.
fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Resolves one interpolation variable from the environment visible to this layer.
fn lookup<'a>(
    name: &str,
    location: Location,
    environment: &'a Environment,
) -> Result<&'a str, TomlInterpolationError> {
    let Some(value) = environment.raw(name) else {
        return Err(TomlInterpolationError::Missing { name: name.to_owned(), location });
    };
    value
        .to_str()
        .ok_or_else(|| TomlInterpolationError::NonUtf8 { name: name.to_owned(), location })
}

/// Pushes arbitrary UTF-8 as safe TOML basic-string content.
fn push_basic_string_value(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '\u{0008}' => output.push_str("\\b"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\u{000C}' => output.push_str("\\f"),
            '\r' => output.push_str("\\r"),
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            character if character <= '\u{001F}' || character == '\u{007F}' => {
                let code = u32::from(character);
                if code <= 0xFFFF {
                    let _ = write!(output, "\\u{code:04X}");
                } else {
                    let _ = write!(output, "\\U{code:08X}");
                }
            }
            character => output.push(character),
        }
    }
}

/// Returns the character beginning at `index` and the following byte offset.
fn next_char(input: &str, index: usize) -> (char, usize) {
    let character = input[index..].chars().next().expect("index must be within input");
    (character, index + character.len_utf8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_uses_the_visible_environment() {
        let environment = Environment::from_pairs(&[("VALUE", "visible")]);
        let expansion = expand_toml("value = \"${VALUE}\"\n", &environment)
            .expect("placeholder should resolve");

        assert_eq!(expansion.text, "value = \"visible\"\n");
        assert!(expansion.substituted);
    }

    #[test]
    fn bare_placeholders_are_rejected() {
        let process = Environment::from_pairs(&[("WORKERS", "8")]);
        let error = expand_toml("workers = ${WORKERS}\n", &process)
            .expect_err("placeholders outside TOML strings must be rejected");

        assert!(matches!(error, TomlInterpolationError::Structural { .. }));
        assert!(error.to_string().contains("top-level basic string value"));
    }

    #[test]
    fn placeholders_in_keys_tables_and_collection_fragments_are_rejected() {
        let process = Environment::from_pairs(&[("KEY", "name"), ("VALUE", "value")]);

        assert!(matches!(
            expand_toml("\"${KEY}\" = 1\n", &process),
            Err(TomlInterpolationError::Structural { .. })
        ));
        assert!(matches!(
            expand_toml("[${KEY}]\nvalue = 1\n", &process),
            Err(TomlInterpolationError::Structural { .. })
        ));
        assert!(matches!(
            expand_toml("values = [\"${VALUE}\"]\n", &process),
            Err(TomlInterpolationError::Structural { .. })
        ));
    }

    #[test]
    fn basic_string_interpolation_escapes_inserted_values() {
        let process = Environment::from_pairs(&[("VALUE", "quote=\"yes\"\\path\nnext")]);
        let expansion = expand_toml("value = \"${VALUE}\"\n", &process)
            .expect("basic-string interpolation should be escaped");

        assert_eq!(expansion.text, "value = \"quote=\\\"yes\\\"\\\\path\\nnext\"\n");
    }

    #[test]
    fn comments_and_escaped_placeholders_are_left_literal() {
        let process = Environment::from_pairs(&[]);
        let expansion = expand_toml("value = \"$${MISSING}\" # ${ALSO_MISSING}\n", &process)
            .expect("literal placeholders should not require environment values");

        assert_eq!(expansion.text, "value = \"${MISSING}\" # ${ALSO_MISSING}\n");
        assert!(!expansion.substituted);
    }

    #[test]
    fn interpolation_errors_report_physical_line_and_column() {
        let input = "workers = 8\nendpoint = \"${MISSING}\"\n";
        let error = expand_toml(input, &Environment::default())
            .expect_err("missing interpolation values should fail");

        assert!(matches!(
            error,
            TomlInterpolationError::Missing { location: Location { line: 2, column: 13 }, .. }
        ));
    }

    #[test]
    fn missing_and_malformed_placeholders_fail_without_values() {
        let process = Environment::from_pairs(&[]);
        let missing = expand_toml("value = \"${MISSING}\"\n", &process)
            .expect_err("missing variables must fail");
        assert!(missing.to_string().contains("`MISSING`"));

        let malformed = expand_toml("value = ${BROKEN\n", &process)
            .expect_err("unterminated placeholders must fail");
        assert!(malformed.to_string().contains("unterminated environment placeholder"));
    }

    #[test]
    fn literal_strings_reject_environment_interpolation() {
        let process = Environment::from_pairs(&[("VALUE", "secret")]);
        let error = expand_toml("value = '${VALUE}'\n", &process)
            .expect_err("literal TOML strings cannot safely escape arbitrary environment values");

        assert!(error.to_string().contains("use a basic string instead"));
    }
}
