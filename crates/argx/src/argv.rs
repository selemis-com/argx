//! Raw command-line token binding against static command metadata.

use std::ffi::OsStr;

use crate::__private::{Arg, Command, Flag};

/// Synthetic metadata for the built-in help switch.
static HELP_FLAG: Flag<'static> = Flag {
    key: 0,
    name: "help",
    help: Some("Print help"),
    longs: &["help"],
    shorts: b"h",
    ..Flag::BOOL
};

/// One token binding produced by the raw argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'t, 'v> {
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
    /// Built-in help was requested for the current command scope.
    DisplayHelp,
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
    /// A help request or error is terminal: subsequent calls return `None`. Events emitted before a
    /// terminal result are a partial parse and must be discarded by callers. A short bundle is
    /// preflighted before its first event, so an unknown short rejects the whole token atomically.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DisplayHelp`] for the built-in help switch, or a structured error when a
    /// token cannot be bound according to the static command metadata.
    pub fn next_event(&mut self) -> Option<Result<Event<'t, 'v>, Error<'t, 'v>>> {
        if self.done {
            return None;
        }

        let event = self.step();
        if event.as_ref().is_some_and(Result::is_err) {
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

        let declared_numeric_short = matches!(token, [b'-', short]
            if short.is_ascii_digit() && self.find_short(*short).is_some());
        if !declared_numeric_short
            && is_negative_number(token)
            && self.next_arg().is_some_and(|argument| argument.allow_negative_numbers)
        {
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
        if name == b"help" {
            return if attached.is_some() {
                Err(Error::UnexpectedFlagValue { flag: &HELP_FLAG })
            } else {
                Err(Error::DisplayHelp)
            };
        }
        let Some(flag) = self.find_long(name) else {
            return Err(Error::UnknownFlag { token });
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
            if short == b'h' {
                remaining = tail;
                continue;
            }
            match self.find_short(short) {
                None => return Err(Error::UnknownFlag { token }),
                Some(flag) if flag.takes_value => return Ok(()),
                Some(_) => remaining = tail,
            }
        }
        Ok(())
    }

    /// Emits one flag from the current short bundle.
    fn short_flag(&mut self) -> Result<Event<'t, 'v>, Error<'t, 'v>> {
        let Some((&short, rest)) = self.bundle.split_first() else {
            return Err(Error::UnknownFlag { token: self.bundle_token });
        };
        if short == b'h' {
            self.bundle = &[];
            return Err(Error::DisplayHelp);
        }
        let Some(flag) = self.find_short(short) else {
            self.bundle = &[];
            return Err(Error::UnknownFlag { token: self.bundle_token });
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
    fn next_arg(&self) -> Option<&'t Arg<'t>> {
        self.command.args.get(self.arg_position).copied()
    }

    /// Looks up one child command by exact command-line spelling.
    fn find_subcommand(&self, name: &[u8]) -> Option<&'t Command<'t>> {
        self.command.subcommands.iter().copied().find(|command| command.name.as_bytes() == name)
    }

    /// Looks up a long flag using the current command before inherited globals.
    ///
    /// Ancestors are searched nearest-first. This gives descendant declarations normal lexical
    /// shadowing behavior while globals remain owned by the command that declared them.
    fn find_long(&self, name: &[u8]) -> Option<&'t Flag<'t>> {
        self.command
            .flags
            .iter()
            .copied()
            .find(|flag| flag.longs.iter().any(|long| long.as_bytes() == name))
            .or_else(|| {
                self.ancestors.iter().rev().find_map(|command| {
                    command.flags.iter().copied().find(|flag| {
                        flag.global && flag.longs.iter().any(|long| long.as_bytes() == name)
                    })
                })
            })
    }

    /// Looks up one short spelling using the current command before inherited globals.
    fn find_short(&self, short: u8) -> Option<&'t Flag<'t>> {
        self.command
            .flags
            .iter()
            .copied()
            .find(|flag| flag.shorts.contains(&short))
            .or_else(|| {
                self.ancestors.iter().rev().find_map(|command| {
                    command
                        .flags
                        .iter()
                        .copied()
                        .find(|flag| flag.global && flag.shorts.contains(&short))
                })
            })
    }

    /// Returns the selected command chain from the root through the current command.
    pub(crate) fn command_path(&self) -> impl Iterator<Item = &'t Command<'t>> + '_ {
        self.ancestors.iter().copied().chain(std::iter::once(self.command))
    }
}

/// Views an operating-system argument as its self-synchronizing encoded bytes.
fn bytes(value: &OsStr) -> &[u8] {
    value.as_encoded_bytes()
}

/// Reports whether a token is syntactically flag-like.
///
/// A lone `-` remains a value, conventionally representing standard input.
const fn is_flag_like(token: &[u8]) -> bool {
    matches!(token, [b'-', rest @ ..] if !rest.is_empty())
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
