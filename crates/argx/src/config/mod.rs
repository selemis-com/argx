//! Layered typed configuration.
//!
//! Derive `argx::Config`, create its generated `loader()`, and add the sources the application
//! wants in increasing precedence order. Later layers override only values they provide. Defaults
//! are explicit through [`Defaults`], and Argx does not discover configuration files implicitly.
//!
//! See the crate-level configuration guide for field attributes and layer behavior.

mod dotenv;
mod environment;
mod error;
mod loader;
#[cfg(feature = "toml")]
mod toml;

pub use error::{Error as SourceError, Source};
#[cfg(feature = "toml")]
#[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
pub use loader::Toml;
pub use loader::{Argv, Defaults, Dotenv, Environment, Layer, Loader};

/// Failure while resolving a configuration from its declared layers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Command-line input could not be parsed.
    #[error(transparent)]
    Arguments(#[from] crate::Error),
    /// A non-CLI configuration layer could not be resolved.
    #[error(transparent)]
    Source(#[from] SourceError),
}

#[doc(hidden)]
pub mod __private {
    pub use serde;

    pub use super::{
        environment::{
            Environment, EnvironmentContract, EnvironmentError, parse_environment_delimited_field,
            parse_environment_field,
        },
        loader::Config,
    };

    #[cfg(feature = "toml")]
    pub type TomlError = toml_edit::de::Error;

    #[must_use]
    pub fn environment_name(prefix: &str, field: &str) -> String {
        let mut name = String::with_capacity(prefix.len() + 1 + field.len());
        name.push_str(prefix);
        name.push('_');
        name.push_str(field);
        name
    }
}
