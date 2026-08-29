//! Unified application configuration resolution.
//!
//! Configuration is resolved from an explicitly ordered stack of layers. Each
//! layer contributes sparse values for the same derived configuration, and later
//! layers override only values they supply. Applications consume only the final
//! resolved value.

mod dotenv;
mod environment;
mod error;
mod loader;
mod toml;

pub use error::{Error as SourceError, Source};
pub use loader::{Argv, Defaults, Dotenv, Environment, Layer, Loader, Toml};

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

    pub use super::loader::Config;

    pub use super::environment::{
        Environment, EnvironmentContract, EnvironmentError, parse_environment_field,
    };

    pub type TomlError = toml_edit::de::Error;

    #[must_use]
    pub fn environment_name(prefix: &str, field: &str) -> String {
        let mut name = String::with_capacity(prefix.len() + 1 + field.len());
        name.push_str(prefix);
        name.push('_');
        name.push_str(field);
        name
    }

    pub trait TomlInput: serde::de::DeserializeOwned + Sized {
        type Config: Config<__Toml = Self>;
        fn into_overrides(self) -> <Self::Config as Config>::Overrides;
    }

    pub trait ConfigState: Default + Sized {
        type Config: Config<Overrides = Self>;
        fn merge(&mut self, higher: Self);
    }
}
