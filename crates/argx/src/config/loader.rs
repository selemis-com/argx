//! Configuration contracts and ordered layer resolution.

use std::{
    ffi::OsString,
    fmt, fs,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use toml_edit::de as toml;

use crate::config::{
    __private,
    dotenv::load_dotenv,
    environment::{Environment, EnvironmentContract, EnvironmentError},
    error::{EnvironmentScope, Error as SourceError, Source},
    toml::expand_toml,
};

/// A typed configuration contract.
pub trait Config: Sized {
    /// The generated sparse representation used while resolving layers.
    type Overrides: __private::ConfigState<Config = Self>;

    /// The generated sparse TOML input representation.
    #[doc(hidden)]
    type __Toml: __private::TomlInput<Config = Self>;

    /// Generated sparse command-line adapter.
    #[doc(hidden)]
    type __CliArgs: crate::__private::CommandArgs;

    /// Creates an empty ordered configuration loader.
    fn loader() -> Loader<Self> {
        Loader { layers: Vec::new(), marker: PhantomData }
    }

    /// Parses explicit argv values into the generated sparse configuration state.
    #[doc(hidden)]
    fn __parse_cli<I, T>(argv: I) -> Result<Self::Overrides, crate::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>;

    /// Converts a generated sparse CLI adapter into configuration state.
    #[doc(hidden)]
    fn __cli_overrides(args: Self::__CliArgs) -> Self::Overrides;

    /// Produces the declared typed defaults state.
    #[doc(hidden)]
    fn __defaults() -> Self::Overrides;

    /// Decodes one TOML scope into typed values for this configuration.
    #[doc(hidden)]
    fn __toml(input: &str) -> Result<Self::Overrides, __private::TomlError> {
        let input = toml::from_str::<Self::__Toml>(input)?;
        Ok(<Self::__Toml as __private::TomlInput>::into_overrides(input))
    }

    /// Returns the generated environment binding contract for this configuration.
    #[doc(hidden)]
    fn __environment_contract(inherited_prefix: Option<&str>) -> EnvironmentContract;

    /// Decodes one environment-like scope with an inherited nested prefix.
    #[doc(hidden)]
    fn __environment_with_prefix(
        environment: &Environment,
        inherited_prefix: Option<&str>,
    ) -> Result<Self::Overrides, EnvironmentError>;

    /// Converts the merged typed state into the resolved configuration value.
    #[doc(hidden)]
    fn __finalize(resolved: Self::Overrides) -> Result<Self, SourceError>;
}

/// A configuration layer accepted by [`Loader::layer`].
///
/// Built-in layer types convert into this opaque wrapper. Layers are applied
/// in declaration order, and later layers replace only values they supply.
#[derive(Clone, Debug)]
pub struct Layer(LayerKind);

/// Declared field defaults.
#[derive(Clone, Copy, Debug, Default)]
pub struct Defaults;

/// One TOML file layer.
#[derive(Clone, Debug)]
pub struct Toml {
    /// Filesystem path read by this layer.
    path: PathBuf,
}

impl Toml {
    /// Creates a TOML layer from one filesystem path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

/// One dotenv-format file layer.
#[derive(Clone, Debug)]
pub struct EnvFile {
    /// Filesystem path read by this layer.
    path: PathBuf,
}

impl EnvFile {
    /// Creates an environment-file layer from one filesystem path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

/// The current process environment.
#[derive(Clone, Copy, Debug, Default)]
pub struct Env;

/// One command-line layer.
#[derive(Clone, Debug)]
pub struct Argv {
    /// Complete argument vector including the program name.
    values: Vec<OsString>,
}

impl Argv {
    /// Captures the current process argument vector.
    #[must_use]
    pub fn current() -> Self {
        Self { values: std::env::args_os().collect() }
    }

    /// Creates an argv layer from a complete argument vector whose first item is
    /// the program name.
    #[must_use]
    pub fn new<I, T>(argv: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self { values: argv.into_iter().map(Into::into).collect() }
    }
}

impl From<Defaults> for Layer {
    fn from(_: Defaults) -> Self {
        Self(LayerKind::Defaults)
    }
}

impl From<Toml> for Layer {
    fn from(layer: Toml) -> Self {
        Self(LayerKind::Toml(layer))
    }
}

impl From<EnvFile> for Layer {
    fn from(layer: EnvFile) -> Self {
        Self(LayerKind::EnvFile(layer))
    }
}

impl From<Env> for Layer {
    fn from(_: Env) -> Self {
        Self(LayerKind::Env)
    }
}

impl From<Argv> for Layer {
    fn from(layer: Argv) -> Self {
        Self(LayerKind::Argv(layer))
    }
}

#[derive(Clone, Debug)]
/// Internal representation of the built-in configuration layers.
enum LayerKind {
    /// Declared field defaults.
    Defaults,
    /// One TOML file.
    Toml(Toml),
    /// One dotenv-format file.
    EnvFile(EnvFile),
    /// The current process environment.
    Env,
    /// One complete command-line argument vector.
    Argv(Argv),
}

/// Resolves a configuration from an ordered stack of layers.
#[must_use = "a configuration loader has no effect until it is resolved"]
pub struct Loader<C: Config> {
    /// Layers to apply in declaration order.
    layers: Vec<Layer>,
    /// Associates the loader with its resolved configuration type.
    marker: PhantomData<fn() -> C>,
}

impl<C: Config> fmt::Debug for Loader<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Loader").field("layers", &self.layers.len()).finish()
    }
}

impl<C: Config> Loader<C> {
    /// Appends one configuration layer.
    ///
    /// Layers are resolved in call order, so later layers have higher
    /// precedence for values they provide.
    pub fn layer<L: Into<Layer>>(mut self, layer: L) -> Self {
        self.layers.push(layer.into());
        self
    }

    /// Resolves all configured layers into the final configuration value.
    ///
    /// # Errors
    /// Returns an error when a layer cannot be loaded or decoded, command-line
    /// parsing fails, or a required field remains unresolved.
    pub fn resolve(self) -> Result<C, super::Error> {
        let environment_contract = C::__environment_contract(None);
        environment_contract.validate().map_err(SourceError::environment_contract)?;

        let mut state = C::Overrides::default();
        let mut environment = Environment::default();

        for layer in self.layers {
            match layer.0 {
                LayerKind::Defaults => {
                    __private::ConfigState::merge(&mut state, C::__defaults());
                }
                LayerKind::Toml(layer) => {
                    merge_toml::<C>(&mut state, &layer.path, &environment)?;
                }
                LayerKind::EnvFile(layer) => {
                    let higher =
                        load_dotenv(&layer.path, &environment).map_err(SourceError::dotenv)?;
                    merge_environment::<C>(
                        &mut state,
                        &higher,
                        EnvironmentScope::File(layer.path),
                        &environment_contract,
                    )?;
                    environment.overlay(higher);
                }
                LayerKind::Env => {
                    let higher = Environment::process();
                    merge_environment::<C>(
                        &mut state,
                        &higher,
                        EnvironmentScope::Process,
                        &environment_contract,
                    )?;
                    environment.overlay(higher);
                }
                LayerKind::Argv(layer) => {
                    let higher = C::__parse_cli(layer.values)?;
                    __private::ConfigState::merge(&mut state, higher);
                }
            }
        }

        C::__finalize(state).map_err(super::Error::from)
    }
}

/// Reads, interpolates, decodes, and merges one TOML layer.
fn merge_toml<C: Config>(
    resolved: &mut C::Overrides,
    path: &Path,
    environment: &Environment,
) -> Result<(), SourceError> {
    let contents = fs::read_to_string(path)
        .map_err(|source| SourceError::read_toml(Source::Toml, path, source))?;

    let expansion = expand_toml(&contents, environment)
        .map_err(|error| SourceError::interpolate_toml(Source::Toml, path, error))?;
    let higher = C::__toml(&expansion.text).map_err(|error| {
        SourceError::parse_toml(
            Source::Toml,
            path,
            &contents,
            &expansion.text,
            error,
            expansion.substituted,
        )
    })?;
    __private::ConfigState::merge(resolved, higher);
    Ok(())
}

/// Decodes and merges one environment-like layer into the typed resolution state.
fn merge_environment<C: Config>(
    resolved: &mut C::Overrides,
    environment: &Environment,
    scope: EnvironmentScope,
    contract: &EnvironmentContract,
) -> Result<(), SourceError> {
    if let Some(variable) = contract.unknown_variable(environment) {
        return Err(SourceError::unknown_environment(scope, variable));
    }
    let higher = C::__environment_with_prefix(environment, None)
        .map_err(|source| SourceError::environment(scope, source))?;
    __private::ConfigState::merge(resolved, higher);
    Ok(())
}
