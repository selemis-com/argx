//! Raw command-line token binding against static command metadata.
//!
//! This layer implements only lexical command-line grammar: scope selection, option spellings,
//! short bundles, attached versus detached values, positional routing, and `--`. It deliberately
//! does not enforce typed occurrence counts, consult environment variables, apply defaults, or
//! convert values. Keeping those concerns out of the raw parser gives syntax errors deterministic
//! precedence over later binding failures.
//!
//! Events borrow both the generated static command tables and the caller's `argv`; owned values are
//! created only by the typed binding layer once the lexical parse has succeeded.

use std::ffi::OsStr;

use crate::__private::{Action, Arg, Command, Flag, Named, resolve_long, resolve_short};

/// One token binding produced by the raw argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'t, 'v> {
    /// A built-in parser action was selected.
    Action {
        /// Static metadata for the matched action.
        action: &'t Action<'t>,
        /// Whether the long spelling selected the action.
        long: bool,
    },
    /// A named flag was matched.
    Flag {
        /// Static metadata for the matched flag.
        flag: &'t Flag<'t>,
        /// Encoded value consumed by the flag, when it takes one.
        value: Option<&'v [u8]>,
    },
    /// A positional argument received one value.
    Arg {
        /// Static metadata for the matched positional argument.
        arg: &'t Arg<'t>,
        /// Encoded value bound to the positional argument.
        value: &'v [u8],
    },
    /// A nested command was selected.
    Command {
        /// Static metadata for the selected child command.
        command: &'t Command<'t>,
    },
}

/// A failure while binding command-line tokens to static argument metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error<'t, 'v> {
    /// A value was attached to a built-in action that does not accept one.
    UnexpectedActionValue {
        /// Static metadata for the action receiving the value.
        action: &'t Action<'t>,
    },
    /// A flag-like token did not match any declared flag.
    UnknownFlag {
        /// Whole encoded token supplied by the caller.
        token: &'v [u8],
    },
    /// A flag that consumes a value did not receive one.
    MissingFlagValue {
        /// Static metadata for the flag missing its value.
        flag: &'t Flag<'t>,
    },
    /// A value was attached to a switch that does not accept one.
    UnexpectedFlagValue {
        /// Static metadata for the flag receiving the value.
        flag: &'t Flag<'t>,
    },
    /// A word could not be assigned to any positional argument.
    UnexpectedArg {
        /// Whole encoded token supplied by the caller.
        token: &'v [u8],
    },
    /// A word was encountered where one of the current command's child commands was expected.
    UnknownCommand {
        /// Whole encoded token supplied by the caller.
        token: &'v [u8],
    },
}

/// Single-pass parser over command-line arguments that exclude the program name.
///
/// The parser maintains the selected command path so local options can shadow inherited global
/// options. A terminal action or error stops iteration; callers must not treat earlier events as a
/// committed parse result until the complete token stream succeeds.
#[derive(Debug)]
pub struct ArgvParser<'t, 'a, 'v> {
    /// Static command definition used for token matching.
    command: &'t Command<'t>,
    /// Selected command ancestors from root to the parent of `command`.
    ancestors: Vec<&'t Command<'t>>,
    /// Command-line arguments after the program name.
    argv: &'a [&'v OsStr],
    /// Index of the next argument to inspect.
    position: usize,
    /// Index of the positional table entry currently receiving values.
    arg_position: usize,
    /// Bytes left in a short-flag bundle.
    bundle: &'v [u8],
    /// Whole encoded token from which `bundle` was taken.
    bundle_token: &'v [u8],
    /// Whether `--` stopped flag interpretation.
    flags_stopped: bool,
    /// Whether a fatal parse error has already been returned.
    done: bool,
}

impl<'t, 'a, 'v> ArgvParser<'t, 'a, 'v> {
    /// Creates a parser for `argv` against `command`.
    ///
    /// `argv` contains only the command-line arguments; the program name is not included.
    #[must_use]
    pub const fn new(command: &'t Command<'t>, argv: &'a [&'v OsStr]) -> Self {
        Self {
            command,
            ancestors: Vec::new(),
            argv,
            position: 0,
            arg_position: 0,
            bundle: &[],
            bundle_token: &[],
            flags_stopped: false,
            done: false,
        }
    }

    /// Produces the next token binding or terminal parser result.
    ///
    /// A built-in action or error is terminal: subsequent calls return `None`. Events emitted
    /// before a terminal result are a partial parse and must be discarded by callers. A short
    /// bundle is preflighted before its first event, so an unknown short rejects the whole token
    /// atomically.
    ///
    /// # Errors
    ///
    /// Returns a structured error when a token cannot be bound according to the static command
    /// metadata.
    pub fn next_event(&mut self) -> Option<Result<Event<'t, 'v>, Error<'t, 'v>>> {
        if self.done {
            return None;
        }

        let event = self.step();
        if matches!(event.as_ref(), Some(Err(_) | Ok(Event::Action { .. }))) {
            self.done = true;
        }
        event
    }

    /// Advances the state machine by one event.
    fn step(&mut self) -> Option<Result<Event<'t, 'v>, Error<'t, 'v>>> {
        if !self.bundle.is_empty() {
            return Some(self.short_flag());
        }

        let token = bytes(self.argv.get(self.position)?);
        self.position += 1;

        if self.flags_stopped {
            return Some(self.word(token));
        }

        if token == b"--" {
            self.flags_stopped = true;
            return self.step();
        }

        if routes_negative_number_to_arg(self.command, &self.ancestors, self.next_arg(), token) {
            return Some(self.word(token));
        }

        if is_flag_like(token) {
            if token.starts_with(b"--") {
                return Some(self.long_flag(token));
            }

            if let Err(error) = self.check_short_bundle(token) {
                return Some(Err(error));
            }
            self.bundle = &token[1..];
            self.bundle_token = token;
            return Some(self.short_flag());
        }

        Some(self.word(token))
    }

    /// Matches a long flag and consumes its value when required.
    fn long_flag(&mut self, token: &'v [u8]) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        let body = &token[2..];
        let (name, attached) = body
            .iter()
            .position(|byte| *byte == b'=')
            .map_or((body, None), |index| (&body[..index], Some(&body[index + 1..])));
        let flag = match resolve_long(self.command, &self.ancestors, name) {
            Some(Named::Action(action)) => {
                return if attached.is_some() {
                    Err(Error::UnexpectedActionValue { action })
                } else {
                    Ok(Event::Action { action, long: true })
                };
            }
            Some(Named::Flag { flag, .. }) => flag,
            None => return Err(Error::UnknownFlag { token }),
        };

        let value = if flag.takes_value {
            match attached {
                Some(value) => Some(value),
                None => Some(self.take_detached_value(flag)?),
            }
        } else if attached.is_some() {
            return Err(Error::UnexpectedFlagValue { flag });
        } else {
            None
        };

        Ok(Event::Flag { flag, value })
    }

    /// Verifies an entire short bundle before any event from it is emitted.
    ///
    /// A value-taking short ends the bundle because every following byte belongs to its value.
    fn check_short_bundle(&self, token: &'v [u8]) -> Result<(), Error<'t, 'v>> {
        let mut remaining = &token[1..];
        while let Some((&short, tail)) = remaining.split_first() {
            match resolve_short(self.command, &self.ancestors, short) {
                Some(Named::Flag { flag, .. }) if flag.takes_value => return Ok(()),
                Some(Named::Action(_) | Named::Flag { .. }) => remaining = tail,
                None => return Err(Error::UnknownFlag { token }),
            }
        }
        Ok(())
    }

    /// Emits one flag from the current short bundle.
    fn short_flag(&mut self) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        let Some((&short, rest)) = self.bundle.split_first() else {
            return Err(Error::UnknownFlag { token: self.bundle_token });
        };
        let flag = match resolve_short(self.command, &self.ancestors, short) {
            Some(Named::Action(action)) => {
                self.bundle = &[];
                return Ok(Event::Action { action, long: false });
            }
            Some(Named::Flag { flag, .. }) => flag,
            None => {
                self.bundle = &[];
                return Err(Error::UnknownFlag { token: self.bundle_token });
            }
        };

        if !flag.takes_value {
            self.bundle = rest;
            return Ok(Event::Flag { flag, value: None });
        }

        self.bundle = &[];
        let value = if rest.is_empty() {
            self.take_detached_value(flag)?
        } else {
            rest.strip_prefix(b"=").unwrap_or(rest)
        };
        Ok(Event::Flag { flag, value: Some(value) })
    }

    /// Consumes one detached flag value according to the flag's hyphen policy.
    fn take_detached_value(&mut self, flag: &'t Flag<'t>) -> Result<&'v [u8], Error<'t, 'v>> {
        let Some(value) = self.argv.get(self.position).copied().map(bytes) else {
            return Err(Error::MissingFlagValue { flag });
        };
        if !flag.allow_hyphen_values
            && is_flag_like(value)
            && !(flag.allow_negative_numbers && is_negative_number(value))
        {
            return Err(Error::MissingFlagValue { flag });
        }
        self.position += 1;
        Ok(value)
    }

    /// Selects an exact child command or binds a word to the next positional in scope.
    fn word(&mut self, token: &'v [u8]) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        if !self.flags_stopped
            && let Some(command) = self.find_subcommand(token)
        {
            self.ancestors.push(self.command);
            self.command = command;
            self.arg_position = 0;
            return Ok(Event::Command { command });
        }

        let Some(arg) = self.next_arg() else {
            return if !self.flags_stopped && !self.command.subcommands.is_empty() {
                Err(Error::UnknownCommand { token })
            } else {
                Err(Error::UnexpectedArg { token })
            };
        };
        if !arg.variadic {
            self.arg_position += 1;
        }
        Ok(Event::Arg { arg, value: token })
    }

    /// Returns the positional argument that would receive the next word.
    pub(crate) fn next_arg(&self) -> Option<&'t Arg<'t>> {
        self.command.args.get(self.arg_position).copied()
    }

    /// Looks up one child command by exact command-line spelling.
    fn find_subcommand(&self, name: &[u8]) -> Option<&'t Command<'t>> {
        self.command.subcommands.iter().copied().find(|command| {
            command.name.as_bytes() == name
                || command.aliases.iter().any(|alias| alias.as_bytes() == name)
        })
    }

    /// Returns the currently selected command.
    pub(crate) const fn command(&self) -> &'t Command<'t> {
        self.command
    }

    /// Returns the selected command ancestors from root to current parent.
    pub(crate) fn ancestors(&self) -> &[&'t Command<'t>] {
        &self.ancestors
    }

    /// Reports whether `--` has stopped flag interpretation.
    pub(crate) const fn flags_stopped(&self) -> bool {
        self.flags_stopped
    }

    /// Reports whether every supplied argv token has been consumed.
    pub(crate) const fn at_end(&self) -> bool {
        self.position == self.argv.len() && self.bundle.is_empty()
    }

    /// Returns the selected command chain from the root through the current command.
    pub(crate) fn command_path(&self) -> impl DoubleEndedIterator<Item = &'t Command<'t>> + '_ {
        self.ancestors.iter().copied().chain(std::iter::once(self.command))
    }
}

/// Views an operating-system argument as native bytes on supported Unix targets.
#[cfg(unix)]
fn bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt as _;

    value.as_bytes()
}

/// Keeps unsupported non-Unix targets buildable without claiming native-string round trips.
#[cfg(not(unix))]
fn bytes(value: &OsStr) -> &[u8] {
    value.as_encoded_bytes()
}

/// Reports whether a token is syntactically flag-like.
///
/// A lone `-` remains a value, conventionally representing standard input.
const fn is_flag_like(token: &[u8]) -> bool {
    matches!(token, [b'-', rest @ ..] if !rest.is_empty())
}

/// Reports whether one negative-number token routes to the next positional argument.
///
/// An exact declared numeric short flag takes precedence over positional negative-number routing.
/// Keeping this decision shared prevents completion and ordinary parsing from disagreeing about a
/// token such as `-1`.
pub(crate) fn routes_negative_number_to_arg<'t>(
    command: &'t Command<'t>,
    ancestors: &[&'t Command<'t>],
    next_arg: Option<&'t Arg<'t>>,
    token: &[u8],
) -> bool {
    let declared_numeric_short = matches!(token, [b'-', short]
    if short.is_ascii_digit()
        && matches!(
            resolve_short(command, ancestors, *short),
            Some(Named::Flag { .. })
        ));
    !declared_numeric_short
        && is_negative_number(token)
        && next_arg.is_some_and(|argument| argument.allow_negative_numbers)
}

/// Reports whether a flag-like token has the supported negative-number shape.
fn is_negative_number(token: &[u8]) -> bool {
    token.strip_prefix(b"-").is_some_and(is_number)
}

/// Recognizes decimal and scientific-notation number spellings without UTF-8 conversion.
fn is_number(token: &[u8]) -> bool {
    let (mantissa, exponent) = token
        .iter()
        .position(|byte| matches!(byte, b'e' | b'E'))
        .map_or((token, None), |index| (&token[..index], Some(&token[index + 1..])));

    let mut seen_digit = false;
    let mut seen_dot = false;
    for &byte in mantissa {
        match byte {
            b'0'..=b'9' => seen_digit = true,
            b'.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    if !seen_digit {
        return false;
    }

    exponent.is_none_or(|exponent| {
        let digits =
            exponent.strip_prefix(b"+").or_else(|| exponent.strip_prefix(b"-")).unwrap_or(exponent);
        !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
    })
}

#[cfg(test)]
mod tests {
    use super::{is_negative_number, is_number};

    #[test]
    fn recognizes_supported_number_shapes() {
        for value in [
            &b"1"[..],
            &b"1.5"[..],
            &b".5"[..],
            &b"1."[..],
            &b"1e5"[..],
            &b"1e-5"[..],
            &b"1E+5"[..],
        ] {
            assert!(is_number(value), "{value:?}");
        }
        for value in [&b""[..], &b"."[..], &b"e1"[..], &b"1e"[..], &b"1.2.3"[..], &b"1x"[..]] {
            assert!(!is_number(value), "{value:?}");
        }
        assert!(is_negative_number(b"-1"));
        assert!(is_negative_number(b"-1.5e2"));
        assert!(!is_negative_number(b"--1"));
        assert!(!is_negative_number(b"-inf"));
    }
}
