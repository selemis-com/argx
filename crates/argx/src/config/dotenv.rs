//! Dotenv parsing and substitution.

use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Read},
    iter::Peekable,
    path::{Path, PathBuf},
};

use crate::config::{environment::Environment, error::Location};

/// Errors produced while reading or parsing dotenv configuration.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DotenvError {
    /// The environment file could not be opened or read.
    #[error("failed to read dotenv file `{}`: {source}", path.display())]
    Read {
        /// Environment-file path.
        path: PathBuf,
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// A logical dotenv line was malformed.
    #[error(
            "failed to parse dotenv file `{}` at line {}, column {}",
            path.display(),
            .location.line,
            .location.column,
        )]
    ParseSyntax {
        /// Environment-file path.
        path: PathBuf,
        /// Source location of the dotenv failure.
        location: Location,
    },
    /// A dotenv substitution referenced an unavailable variable.
    #[error(
            "dotenv file `{}` references unset environment variable `{variable}` at line {}, column {}",
            path.display(),
            .location.line,
            .location.column,
        )]
    ParseMissingVariable {
        /// Environment-file path.
        path: PathBuf,
        /// Source location of the dotenv failure.
        location: Location,
        /// Missing process and dotenv variable.
        variable: String,
    },
    /// A dotenv substitution referenced a non-UTF-8 process variable.
    #[error(
            "dotenv file `{}` references non-UTF-8 environment variable `{variable}` at line {}, column {}",
            path.display(),
            .location.line,
            .location.column,
        )]
    ParseNonUtf8Variable {
        /// Environment-file path.
        path: PathBuf,
        /// Source location of the dotenv failure.
        location: Location,
        /// Non-UTF-8 process variable.
        variable: String,
    },
}

/// Stable dotenv parse failure categories.
#[derive(Debug)]
enum ParseKind {
    /// Dotenv syntax was malformed.
    Syntax,
    /// A substitution referenced a variable that is not defined.
    MissingVariable(String),
    /// A substitution referenced a process variable that is not valid UTF-8.
    NonUtf8Variable(String),
}

impl DotenvError {
    /// Returns the path associated with this dotenv error, when available.
    pub(crate) fn path(&self) -> Option<&Path> {
        match self {
            Self::Read { path, .. }
            | Self::ParseSyntax { path, .. }
            | Self::ParseMissingVariable { path, .. }
            | Self::ParseNonUtf8Variable { path, .. } => Some(path),
        }
    }

    /// Returns the source location for a dotenv syntax error.
    pub(crate) const fn location(&self) -> Option<Location> {
        match self {
            Self::ParseSyntax { location, .. }
            | Self::ParseMissingVariable { location, .. }
            | Self::ParseNonUtf8Variable { location, .. } => Some(*location),
            Self::Read { .. } => None,
        }
    }

    /// Returns the environment variable associated with a substitution failure.
    pub(crate) fn variable(&self) -> Option<&str> {
        match self {
            Self::ParseMissingVariable { variable, .. }
            | Self::ParseNonUtf8Variable { variable, .. } => Some(variable),
            Self::Read { .. } | Self::ParseSyntax { .. } => None,
        }
    }
}

/// Loads and parses one explicit dotenv-format file.
///
/// # Errors
/// Returns an error when the file cannot be read or its contents are invalid.
pub(crate) fn load_dotenv(path: &Path, process: &Environment) -> Result<Environment, DotenvError> {
    let file = File::open(path)
        .map_err(|source| DotenvError::Read { path: path.to_path_buf(), source })?;
    let values = parse(file, process).map_err(|source| match source {
        ParseError::Io(source) => DotenvError::Read { path: path.to_path_buf(), source },
        ParseError::Line { location, kind: ParseKind::Syntax } => {
            DotenvError::ParseSyntax { path: path.to_path_buf(), location }
        }
        ParseError::Line { location, kind: ParseKind::MissingVariable(variable) } => {
            DotenvError::ParseMissingVariable { path: path.to_path_buf(), location, variable }
        }
        ParseError::Line { location, kind: ParseKind::NonUtf8Variable(variable) } => {
            DotenvError::ParseNonUtf8Variable { path: path.to_path_buf(), location, variable }
        }
    })?;
    Ok(Environment::from_utf8(values))
}

/// Parses dotenv assignments without modifying the process environment.
fn parse<R: Read>(reader: R, process: &Environment) -> Result<HashMap<String, String>, ParseError> {
    let mut lines = QuotedLines { buf: BufReader::new(reader), line: 0 };

    // Strip an optional UTF-8 BOM.
    let buffer = lines.buf.fill_buf().map_err(ParseError::Io)?;
    if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
        lines.buf.consume(3);
    }

    let mut substitution_data = HashMap::new();
    let mut values = HashMap::new();
    for line in lines {
        let line = line?;
        let parsed = LineParser::new(&line.text, &mut substitution_data, process)
            .parse_line()
            .map_err(|error| locate_line_error(error, &line.text, line.start_line))?;
        if let Some((key, value)) = parsed {
            values.insert(key, value);
        }
    }
    Ok(values)
}

/// Internal dotenv parser error before a source path is attached.
#[derive(Debug, thiserror::Error)]
enum ParseError {
    /// Input could not be read.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Dotenv parsing failed at one absolute source location.
    #[error("invalid dotenv assignment")]
    Line {
        /// One-based source location.
        location: Location,
        /// Stable failure category.
        kind: ParseKind,
    },
}

/// Byte offset and stable failure kind within one logical dotenv line.
#[derive(Debug)]
struct LineError {
    /// Byte offset within the logical line.
    index: usize,
    /// Stable failure category.
    kind: ParseKind,
}

impl LineError {
    /// Creates a syntax failure at one byte offset.
    const fn syntax(index: usize) -> Self {
        Self { index, kind: ParseKind::Syntax }
    }

    /// Creates a missing-substitution failure.
    const fn missing(index: usize, variable: String) -> Self {
        Self { index, kind: ParseKind::MissingVariable(variable) }
    }

    /// Creates a non-UTF-8 substitution failure.
    const fn non_utf8(index: usize, variable: String) -> Self {
        Self { index, kind: ParseKind::NonUtf8Variable(variable) }
    }
}

/// Attaches an absolute source location to one logical-line parse error.
fn locate_line_error(error: LineError, input: &str, start_line: usize) -> ParseError {
    let relative = Location::from_offset(input, error.index);
    ParseError::Line {
        location: Location { line: start_line + relative.line - 1, column: relative.column },
        kind: error.kind,
    }
}

/// Iterator over logical dotenv lines with quoted multiline values joined.
struct QuotedLines<B> {
    /// Buffered source of physical input lines.
    buf: B,
    /// Number of physical lines already consumed.
    line: usize,
}

/// One logical dotenv line and its one-based starting physical line.
struct LogicalLine {
    /// Joined logical-line contents.
    text: String,
    /// One-based physical line at which the logical line begins.
    start_line: usize,
}

/// Parser state used while scanning for the end of a logical dotenv line.
#[derive(Clone, Copy, Debug)]
enum ParseState {
    /// Parser is outside quotes and escapes.
    Complete,
    /// Previous character was an escape outside quotes.
    Escape,
    /// Parser is inside a single-quoted string.
    StrongOpen,
    /// Parser is inside a double-quoted string.
    WeakOpen,
    /// Previous character was an escape inside double quotes.
    WeakOpenEscape,
    /// Parser has entered a trailing comment.
    Comment,
    /// Parser is scanning whitespace after a completed value.
    WhiteSpace,
}

/// Evaluates how one physical input fragment changes logical-line parser state.
fn eval_end_state(previous: ParseState, input: &str) -> (usize, ParseState) {
    let mut state = previous;
    let mut position = 0;

    for (offset, character) in input.char_indices() {
        position = offset;
        state = match state {
            ParseState::WhiteSpace => match character {
                '#' => return (position, ParseState::Comment),
                character
                    if character.is_whitespace() && character != '\n' && character != '\r' =>
                {
                    ParseState::WhiteSpace
                }
                '\\' => ParseState::Escape,
                '"' => ParseState::WeakOpen,
                '\'' => ParseState::StrongOpen,
                _ => ParseState::Complete,
            },
            ParseState::Escape => ParseState::Complete,
            ParseState::Complete => match character {
                character
                    if character.is_whitespace() && character != '\n' && character != '\r' =>
                {
                    ParseState::WhiteSpace
                }
                '\\' => ParseState::Escape,
                '"' => ParseState::WeakOpen,
                '\'' => ParseState::StrongOpen,
                _ => ParseState::Complete,
            },
            ParseState::WeakOpen => match character {
                '\\' => ParseState::WeakOpenEscape,
                '"' => ParseState::Complete,
                _ => ParseState::WeakOpen,
            },
            ParseState::WeakOpenEscape => ParseState::WeakOpen,
            ParseState::StrongOpen => match character {
                '\'' => ParseState::Complete,
                _ => ParseState::StrongOpen,
            },
            ParseState::Comment => panic!("comment state should have returned immediately"),
        };
    }
    (position, state)
}

impl<B: BufRead> Iterator for QuotedLines<B> {
    type Item = Result<LogicalLine, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = String::new();
        let mut state = ParseState::Complete;
        let start_line = self.line + 1;
        loop {
            let buffer_start = buffer.len();
            match self.buf.read_line(&mut buffer) {
                Ok(0) => {
                    return match state {
                        ParseState::Complete => None,
                        _ => {
                            let relative = Location::from_offset(&buffer, buffer.len());
                            Some(Err(ParseError::Line {
                                location: Location {
                                    line: start_line + relative.line - 1,
                                    column: relative.column,
                                },
                                kind: ParseKind::Syntax,
                            }))
                        }
                    };
                }
                Ok(_) => {
                    self.line += 1;
                    if buffer[buffer_start..].trim_start().starts_with('#')
                        && buffer[..buffer_start].is_empty()
                    {
                        return Some(Ok(LogicalLine { text: String::new(), start_line }));
                    }
                    let (position, next_state) = eval_end_state(state, &buffer[buffer_start..]);
                    state = next_state;

                    match state {
                        ParseState::Complete => {
                            buffer.truncate(buffer.trim_end_matches(['\r', '\n']).len());
                            return Some(Ok(LogicalLine { text: buffer, start_line }));
                        }
                        ParseState::Comment => {
                            buffer.truncate(buffer_start + position);
                            return Some(Ok(LogicalLine { text: buffer, start_line }));
                        }
                        ParseState::Escape
                        | ParseState::StrongOpen
                        | ParseState::WeakOpen
                        | ParseState::WeakOpenEscape
                        | ParseState::WhiteSpace => {}
                    }
                }
                Err(source) => return Some(Err(ParseError::Io(source))),
            }
        }
    }
}

/// Parser for one logical dotenv assignment line.
struct LineParser<'a> {
    /// Previously parsed values available to later substitutions.
    substitution_data: &'a mut HashMap<String, Option<String>>,
    /// Process environment, which takes precedence during substitution.
    process: &'a Environment,
    /// Remaining unparsed line slice.
    line: &'a str,
    /// Byte offset into the logical line for diagnostics.
    position: usize,
}

impl<'a> LineParser<'a> {
    /// Creates a parser for one logical line.
    fn new(
        line: &'a str,
        substitution_data: &'a mut HashMap<String, Option<String>>,
        process: &'a Environment,
    ) -> Self {
        Self { substitution_data, process, line: line.trim_end(), position: 0 }
    }

    /// Builds a parse error at the current byte offset.
    const fn error(&self) -> LineError {
        LineError::syntax(self.position)
    }

    /// Parses this line into an optional key/value assignment.
    fn parse_line(&mut self) -> Result<Option<(String, String)>, LineError> {
        self.skip_whitespace();
        if self.line.is_empty() || self.line.starts_with('#') {
            return Ok(None);
        }

        let mut key = self.parse_key()?;
        self.skip_whitespace();

        // `export` may be either an optional prefix or the key itself.
        if key == "export" {
            if self.expect_equal().is_err() {
                key = self.parse_key()?;
                self.skip_whitespace();
                self.expect_equal()?;
            }
        } else {
            self.expect_equal()?;
        }
        self.skip_whitespace();

        if self.line.is_empty() || self.line.starts_with('#') {
            self.substitution_data.insert(key.clone(), None);
            return Ok(Some((key, String::new())));
        }

        let value = parse_value(self.line, self.substitution_data, self.process)
            .map_err(|error| LineError { index: self.position + error.index, kind: error.kind })?;
        self.substitution_data.insert(key.clone(), Some(value.clone()));
        Ok(Some((key, value)))
    }

    /// Parses an environment variable key.
    fn parse_key(&mut self) -> Result<String, LineError> {
        if !self
            .line
            .starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
        {
            return Err(self.error());
        }
        let index = self
            .line
            .find(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
            })
            .unwrap_or(self.line.len());
        self.position += index;
        let key = String::from(&self.line[..index]);
        self.line = &self.line[index..];
        Ok(key)
    }

    /// Consumes the assignment separator.
    fn expect_equal(&mut self) -> Result<(), LineError> {
        if !self.line.starts_with('=') {
            return Err(self.error());
        }
        self.line = &self.line[1..];
        self.position += 1;
        Ok(())
    }

    /// Advances past leading whitespace in the remaining line.
    fn skip_whitespace(&mut self) {
        if let Some(index) = self.line.find(|character: char| !character.is_whitespace()) {
            self.position += index;
            self.line = &self.line[index..];
        } else {
            self.position += self.line.len();
            self.line = "";
        }
    }
}

/// Parses and unescapes one dotenv value, applying variable substitution.
fn parse_value(
    input: &str,
    substitution_data: &HashMap<String, Option<String>>,
    process: &Environment,
) -> Result<String, LineError> {
    let mut strong_quote = false;
    let mut weak_quote = false;
    let mut escaped = false;
    let mut expecting_end = false;
    let mut output = String::new();
    let mut characters = input.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        if expecting_end {
            match character {
                ' ' | '\t' => {}
                '#' => break,
                _ => return Err(LineError::syntax(index)),
            }
            continue;
        }

        if strong_quote {
            if character == '\'' {
                strong_quote = false;
            } else {
                output.push(character);
            }
            continue;
        }

        if escaped {
            match character {
                '\\' | '\'' | '"' | '$' | ' ' => output.push(character),
                'n' => output.push('\n'),
                _ => return Err(LineError::syntax(index)),
            }
            escaped = false;
            continue;
        }

        if weak_quote {
            match character {
                '"' => weak_quote = false,
                '\\' => escaped = true,
                '$' => apply_next_substitution(
                    index,
                    &mut characters,
                    process,
                    substitution_data,
                    &mut output,
                    input.len(),
                )?,
                _ => output.push(character),
            }
            continue;
        }

        match character {
            '\'' => strong_quote = true,
            '"' => weak_quote = true,
            '\\' => escaped = true,
            '$' => apply_next_substitution(
                index,
                &mut characters,
                process,
                substitution_data,
                &mut output,
                input.len(),
            )?,
            ' ' | '\t' => expecting_end = true,
            _ => output.push(character),
        }
    }

    if strong_quote || weak_quote || escaped {
        return Err(LineError::syntax(input.len().saturating_sub(1)));
    }

    Ok(output)
}

/// Consumes one `$NAME` or `${NAME}` substitution after the leading `$`.
fn apply_next_substitution<I>(
    dollar_index: usize,
    characters: &mut Peekable<I>,
    process: &Environment,
    substitution_data: &HashMap<String, Option<String>>,
    output: &mut String,
    input_len: usize,
) -> Result<(), LineError>
where
    I: Iterator<Item = (usize, char)>,
{
    let mut name = String::new();
    let braced = matches!(characters.peek(), Some((_, '{')));

    if braced {
        let _ = characters.next();
        let mut closed = false;
        for (_, character) in characters.by_ref() {
            if character == '}' {
                closed = true;
                break;
            }
            name.push(character);
        }
        if !closed {
            return Err(LineError::syntax(input_len.saturating_sub(1)));
        }
        if name.is_empty() {
            return Err(LineError::syntax(dollar_index));
        }
    } else {
        while let Some((_, character)) = characters.peek() {
            if character.is_ascii_alphanumeric() || *character == '_' {
                name.push(*character);
                let _ = characters.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            output.push('$');
            return Ok(());
        }
    }

    apply_substitution(process, substitution_data, dollar_index, &name, output)
}

/// Appends one resolved substitution value to the parsed output.
fn apply_substitution(
    process: &Environment,
    substitution_data: &HashMap<String, Option<String>>,
    index: usize,
    name: &str,
    output: &mut String,
) -> Result<(), LineError> {
    if let Some(value) = process.raw(name) {
        let Some(value) = value.to_str() else {
            return Err(LineError::non_utf8(index, name.to_owned()));
        };
        output.push_str(value);
        return Ok(());
    }

    if let Some(value) = substitution_data.get(name) {
        if let Some(value) = value {
            output.push_str(value);
        }
        return Ok(());
    }

    Err(LineError::missing(index, name.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// Builds a deterministic environment scope from UTF-8 string pairs.
    fn environment(values: &[(&str, &str)]) -> Environment {
        Environment::from_utf8(
            values.iter().map(|(key, value)| (String::from(*key), String::from(*value))).collect(),
        )
    }

    #[test]
    fn parser_supports_quotes_export_comments_bom_and_multiline_values() {
        let input = concat!(
            "\u{feff}export FIRST=plain # comment\n",
            "SECOND=\"two words\"\n",
            "THIRD='three words'\n",
            "MULTI=\"first\nsecond\"\n",
        );
        let values = parse(Cursor::new(input.as_bytes()), &Environment::default())
            .expect("dotenv should parse");

        assert_eq!(values.get("FIRST").map(String::as_str), Some("plain"));
        assert_eq!(values.get("SECOND").map(String::as_str), Some("two words"));
        assert_eq!(values.get("THIRD").map(String::as_str), Some("three words"));
        assert_eq!(values.get("MULTI").map(String::as_str), Some("first\nsecond"));
    }

    #[test]
    fn parser_reports_physical_line_and_column() {
        let input = "GOOD=1\nBROKEN=value extra\n";
        let error = parse(Cursor::new(input.as_bytes()), &Environment::default())
            .expect_err("malformed dotenv syntax should fail");

        assert!(matches!(
            error,
            ParseError::Line {
                location: Location { line: 2, column: 14 },
                kind: ParseKind::Syntax
            }
        ));
    }

    #[test]
    fn substitution_prefers_process_then_latest_preceding_dotenv_assignment() {
        let input = concat!(
            "BASE=first\n",
            "BASE=second\n",
            "FROM_DOTENV=$BASE\n",
            "FROM_PROCESS=${SHADOW}\n",
        );
        let values = parse(Cursor::new(input.as_bytes()), &environment(&[("SHADOW", "process")]))
            .expect("dotenv substitutions should parse");

        assert_eq!(values.get("FROM_DOTENV").map(String::as_str), Some("second"));
        assert_eq!(values.get("FROM_PROCESS").map(String::as_str), Some("process"));
    }

    #[test]
    fn undefined_substitutions_are_errors_instead_of_empty_strings() {
        let error = parse(Cursor::new(b"VALUE=$MISSING\n"), &Environment::default())
            .expect_err("undefined dotenv substitutions must fail");

        assert!(matches!(
            error,
            ParseError::Line {
                location: Location { line: 1, column: 7 },
                kind: ParseKind::MissingVariable(variable),
            } if variable == "MISSING"
        ));
    }

    #[test]
    fn explicitly_empty_dotenv_assignments_remain_valid_substitution_values() {
        let values =
            parse(Cursor::new(b"EMPTY=\nVALUE=before${EMPTY}after\n"), &Environment::default())
                .expect("defined empty values should substitute as empty strings");

        assert_eq!(values.get("VALUE").map(String::as_str), Some("beforeafter"));
    }

    #[test]
    fn unterminated_quotes_are_rejected() {
        assert!(parse(Cursor::new(b"VALUE=\"unterminated\n"), &Environment::default()).is_err());
    }
}
