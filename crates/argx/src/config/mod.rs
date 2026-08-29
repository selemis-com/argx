//! Unified application configuration resolution.
//!
//! Derive `argx::Config`, create the generated `Config::loader()`, and append only the layers the
//! application wants. Layers are sparse and resolved in call order: later layers override only
//! values they supply. Declared defaults participate only through [`Defaults`]; Argx does not
//! discover configuration or dotenv files implicitly.
//!
//! See the crate-level configuration guide for field attributes, environment naming,
//! interpolation, and argv behavior.

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
            Environment, EnvironmentContract, EnvironmentError, parse_environment_field,
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
