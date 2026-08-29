//! Public loading diagnostics and their internal context.

use std::{
    fmt,
    path::{Path, PathBuf},
};

#[cfg(feature = "toml")]
use std::io;
#[cfg(feature = "toml")]
use toml_edit::de as toml;

use crate::config::{
    dotenv::DotenvError,
    environment::{EnvironmentContractError, EnvironmentError},
};
#[cfg(feature = "toml")]
use crate::config::toml::TomlInterpolationError;

/// One-based source location used by configuration diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Location {
    /// One-based line number.
    pub(crate) line: usize,
    /// One-based character column.
    pub(crate) column: usize,
}

impl Location {
    /// Computes a one-based line and character column for a byte offset.
    pub(crate) fn from_offset(input: &str, offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;
        for (index, character) in input.char_indices() {
            if index >= offset {
                break;
            }
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Self { line, column }
    }
}

/// A configuration source that can originate a loading error.
///
/// This describes externally loaded sources only. Declared defaults and explicit
/// overrides cannot produce source-loading diagnostics and therefore are not
/// represented here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Source {
    /// A TOML file layer.
    #[cfg(feature = "toml")]
    Toml,
    /// A dotenv-format file layer.
    Dotenv,
    /// Process environment layer.
    Environment,
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "toml")]
            Self::Toml => formatter.write_str("TOML"),
            Self::Dotenv => formatter.write_str("environment file"),
            Self::Environment => formatter.write_str("process environment"),
        }
    }
}

/// An error produced while loading or resolving configuration.
///
/// Errors retain structured context about the configuration [`Source`],
/// field, environment variable, path, and source location when those concepts
/// apply. Raw environment values are never retained for diagnostics.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error {
    /// Internal error details kept private so parser dependencies do not become
    /// part of Argx's public error representation.
    kind: Box<ErrorKind>,
}

impl Error {
    /// Creates a missing-value resolution error.
    #[doc(hidden)]
    #[must_use]
    pub fn __missing_value(field: &'static str) -> Self {
        Self { kind: Box::new(ErrorKind::MissingValue { field: String::from(field) }) }
    }

    /// Qualifies a nested resolution error with its parent field.
    #[doc(hidden)]
    #[must_use]
    pub fn __within(mut self, parent: &'static str) -> Self {
        if let ErrorKind::MissingValue { field } = self.kind.as_mut() {
            field.insert(0, '.');
            field.insert_str(0, parent);
        }
        self
    }

    /// Creates a TOML read error.
    #[cfg(feature = "toml")]
    pub(crate) fn read_toml(scope: Source, path: &Path, source: io::Error) -> Self {
        Self { kind: Box::new(ErrorKind::ReadToml { scope, path: path.to_path_buf(), source }) }
    }

    /// Creates a TOML interpolation error.
    #[cfg(feature = "toml")]
    pub(crate) fn interpolate_toml(
        scope: Source,
        path: &Path,
        source: TomlInterpolationError,
    ) -> Self {
        Self {
            kind: Box::new(ErrorKind::InterpolateToml { scope, path: path.to_path_buf(), source }),
        }
    }

    /// Creates a TOML decoding error.
    #[cfg(feature = "toml")]
    pub(crate) fn parse_toml(
        scope: Source,
        path: &Path,
        original: &str,
        decoded: &str,
        source: toml::Error,
        interpolated: bool,
    ) -> Self {
        let location = if original == decoded {
            source.span().map(|span| Location::from_offset(original, span.start))
        } else {
            None
        };
        let kind = if interpolated {
            ErrorKind::ParseInterpolatedToml { scope, path: path.to_path_buf() }
        } else {
            ErrorKind::ParseToml { scope, path: path.to_path_buf(), source, location }
        };
        Self { kind: Box::new(kind) }
    }

    /// Creates an environment-file loading or parsing error.
    pub(crate) fn dotenv(source: DotenvError) -> Self {
        Self { kind: Box::new(ErrorKind::Dotenv { source }) }
    }

    /// Creates a typed environment conversion error.
    pub(crate) fn environment(scope: EnvironmentScope, source: EnvironmentError) -> Self {
        Self { kind: Box::new(ErrorKind::Environment { scope, source }) }
    }

    /// Creates an invalid generated environment contract error.
    pub(crate) fn environment_contract(source: EnvironmentContractError) -> Self {
        Self { kind: Box::new(ErrorKind::EnvironmentContract { source }) }
    }

    /// Creates an unknown variable error inside an owned environment prefix.
    pub(crate) fn unknown_environment(scope: EnvironmentScope, variable: String) -> Self {
        Self { kind: Box::new(ErrorKind::UnknownEnvironment { scope, variable }) }
    }

    /// Returns the configuration source associated with this error.
    #[must_use]
    pub fn configuration_source(&self) -> Option<Source> {
        match self.kind.as_ref() {
            ErrorKind::MissingValue { .. } | ErrorKind::EnvironmentContract { .. } => None,
            #[cfg(feature = "toml")]
            ErrorKind::ReadToml { scope, .. }
            | ErrorKind::InterpolateToml { scope, .. }
            | ErrorKind::ParseToml { scope, .. }
            | ErrorKind::ParseInterpolatedToml { scope, .. } => Some(*scope),
            ErrorKind::Dotenv { .. } => Some(Source::Dotenv),
            ErrorKind::Environment { scope, .. } | ErrorKind::UnknownEnvironment { scope, .. } => {
                Some(scope.source())
            }
        }
    }

    /// Returns the dot-qualified Rust configuration field associated with this error.
    #[must_use]
    pub fn field(&self) -> Option<&str> {
        match self.kind.as_ref() {
            ErrorKind::MissingValue { field } => Some(field),
            ErrorKind::Environment { source, .. } => Some(source.field()),
            #[cfg(feature = "toml")]
            ErrorKind::ReadToml { .. }
            | ErrorKind::InterpolateToml { .. }
            | ErrorKind::ParseToml { .. }
            | ErrorKind::ParseInterpolatedToml { .. } => None,
            ErrorKind::Dotenv { .. }
            | ErrorKind::EnvironmentContract { .. }
            | ErrorKind::UnknownEnvironment { .. } => None,
        }
    }

    /// Returns the environment variable associated with this error.
    #[must_use]
    pub fn environment_variable(&self) -> Option<&str> {
        match self.kind.as_ref() {
            #[cfg(feature = "toml")]
            ErrorKind::InterpolateToml { source, .. } => source.variable(),
            ErrorKind::Dotenv { source } => source.variable(),
            ErrorKind::Environment { source, .. } => Some(source.variable()),
            ErrorKind::EnvironmentContract { source } => Some(source.variable()),
            ErrorKind::UnknownEnvironment { variable, .. } => Some(variable),
            ErrorKind::MissingValue { .. } => None,
            #[cfg(feature = "toml")]
            ErrorKind::ReadToml { .. }
            | ErrorKind::ParseToml { .. }
            | ErrorKind::ParseInterpolatedToml { .. } => None,
        }
    }

    /// Returns the filesystem path associated with this error.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self.kind.as_ref() {
            #[cfg(feature = "toml")]
            ErrorKind::ReadToml { path, .. }
            | ErrorKind::InterpolateToml { path, .. }
            | ErrorKind::ParseToml { path, .. }
            | ErrorKind::ParseInterpolatedToml { path, .. } => Some(path),
            ErrorKind::Dotenv { source } => source.path(),
            ErrorKind::Environment { scope, .. } | ErrorKind::UnknownEnvironment { scope, .. } => {
                scope.path()
            }
            ErrorKind::MissingValue { .. } | ErrorKind::EnvironmentContract { .. } => None,
        }
    }

    /// Returns a one-based `(line, column)` source location when available.
    ///
    /// Locations are intentionally omitted when preprocessing changed the TOML
    /// text because parser byte spans would no longer identify the original file.
    #[must_use]
    pub fn location(&self) -> Option<(usize, usize)> {
        let location = match self.kind.as_ref() {
            #[cfg(feature = "toml")]
            ErrorKind::InterpolateToml { source, .. } => Some(source.location()),
            #[cfg(feature = "toml")]
            ErrorKind::ParseToml { location, .. } => *location,
            ErrorKind::Dotenv { source } => source.location(),
            ErrorKind::MissingValue { .. }
            | ErrorKind::Environment { .. }
            | ErrorKind::EnvironmentContract { .. }
            | ErrorKind::UnknownEnvironment { .. } => None,
            #[cfg(feature = "toml")]
            ErrorKind::ReadToml { .. } | ErrorKind::ParseInterpolatedToml { .. } => None,
        }?;
        Some((location.line, location.column))
    }
}

/// Internal configuration error details.
#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    /// A required value was absent after all configured scopes were resolved.
    #[error("missing required configuration value `{field}`")]
    MissingValue {
        /// Dot-qualified Rust field path of the missing configuration value.
        field: String,
    },
    /// A configured TOML path could not be read.
    #[error("failed to read {scope} configuration `{}`: {source}", path.display())]
    #[cfg(feature = "toml")]
    ReadToml {
        /// Scope associated with the path.
        scope: Source,
        /// Path that failed to read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// A configured TOML file could not resolve one environment placeholder.
    #[error("failed to interpolate {scope} configuration `{}`: {source}", path.display())]
    #[cfg(feature = "toml")]
    InterpolateToml {
        /// Scope associated with the path.
        scope: Source,
        /// Path containing the placeholder.
        path: PathBuf,
        /// Placeholder interpolation failure.
        source: TomlInterpolationError,
    },
    /// A configured TOML file could not be decoded into the typed configuration layer.
    #[error("failed to parse {scope} configuration `{}`: {source}", path.display())]
    #[cfg(feature = "toml")]
    ParseToml {
        /// Scope associated with the path.
        scope: Source,
        /// Path containing invalid configuration.
        path: PathBuf,
        /// Underlying TOML decoding error.
        source: toml::Error,
        /// Original-source location when preprocessing did not alter the input.
        location: Option<Location>,
    },
    /// A TOML file could not be decoded after interpolation changed its contents.
    #[error(
        "failed to parse {scope} configuration `{}` after environment interpolation",
        path.display(),
    )]
    #[cfg(feature = "toml")]
    ParseInterpolatedToml {
        /// Scope associated with the path.
        scope: Source,
        /// Path containing invalid interpolated configuration.
        path: PathBuf,
    },
    /// Environment-file reading or syntax failed.
    #[error("failed to load dotenv: {source}")]
    Dotenv {
        /// Underlying dotenv error with path context where available.
        source: DotenvError,
    },
    /// A mapped dotenv or process environment value could not be decoded.
    #[error("failed to parse {scope}: {source}")]
    Environment {
        /// Environment-like scope containing the value.
        scope: EnvironmentScope,
        /// Typed field conversion error.
        source: EnvironmentError,
    },
    /// Generated environment bindings are ambiguous.
    #[error("invalid environment configuration contract: {source}")]
    EnvironmentContract {
        /// Generated contract validation failure.
        source: EnvironmentContractError,
    },
    /// An owned environment prefix contained an undeclared variable.
    #[error("unknown environment variable `{variable}` under a Argx-owned prefix in {scope}")]
    UnknownEnvironment {
        /// Environment-like scope containing the unknown variable.
        scope: EnvironmentScope,
        /// Unknown variable name.
        variable: String,
    },
}

/// Environment-like configuration layer context.
#[derive(Debug)]
pub(crate) enum EnvironmentScope {
    /// Values parsed from an explicit dotenv-format file.
    File(PathBuf),
    /// Values read directly from the process environment.
    Process,
}

impl EnvironmentScope {
    /// Returns the source kind represented by this scope.
    const fn source(&self) -> Source {
        match self {
            Self::File(_) => Source::Dotenv,
            Self::Process => Source::Environment,
        }
    }

    /// Returns the environment-file path associated with this scope, when present.
    fn path(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Process => None,
        }
    }
}

impl fmt::Display for EnvironmentScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => write!(formatter, "environment file `{}`", path.display()),
            Self::Process => formatter.write_str("process environment configuration"),
        }
    }
}
