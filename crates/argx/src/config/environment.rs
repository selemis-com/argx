//! Environment snapshots, binding contracts, and scalar decoding.

use std::{
    collections::{BTreeMap, HashMap},
    env,
    ffi::{OsStr, OsString},
    fmt,
    str::FromStr,
};

use serde::{
    Deserializer,
    de::{self, DeserializeOwned, Visitor, value::StrDeserializer},
};

/// Snapshot of one environment-like configuration scope.
///
/// The type is public only because generated downstream code receives it through
/// Argx's hidden derive protocol.
#[derive(Debug, Default)]
pub struct Environment {
    /// Raw operating-system key/value pairs in this scope.
    values: HashMap<OsString, OsString>,
}

impl Environment {
    /// Captures the process environment without mutating it.
    pub(crate) fn process() -> Self {
        Self { values: env::vars_os().collect() }
    }

    /// Builds an environment scope from UTF-8 dotenv assignments.
    pub(crate) fn from_utf8(values: HashMap<String, String>) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect(),
        }
    }

    /// Returns the raw value associated with one known environment variable.
    pub(crate) fn raw(&self, name: &str) -> Option<&OsStr> {
        self.values.get(OsStr::new(name)).map(OsString::as_os_str)
    }

    /// Overlays another environment-like scope for later interpolation.
    pub(crate) fn overlay(&mut self, higher: Self) {
        self.values.extend(higher.values);
    }
}

/// Environment bindings generated for one configuration contract.
///
/// The type is public only because downstream derive expansions build it through
/// Argx's hidden protocol.
#[derive(Debug, Default)]
pub struct EnvironmentContract {
    /// Field-to-variable bindings in declaration order.
    bindings: Vec<EnvironmentBinding>,
}

/// One resolved environment binding.
#[derive(Debug)]
struct EnvironmentBinding {
    /// Dot-qualified Rust configuration field.
    field: String,
    /// Concrete environment variable name.
    variable: String,
}

impl EnvironmentContract {
    /// Registers one concrete field-to-variable binding.
    #[doc(hidden)]
    pub fn __binding(&mut self, field: &'static str, variable: impl Into<String>) {
        self.bindings
            .push(EnvironmentBinding { field: String::from(field), variable: variable.into() });
    }

    /// Extends this contract with a nested configuration under `parent`.
    #[doc(hidden)]
    pub fn __extend_within(&mut self, mut nested: Self, parent: &'static str) {
        for binding in &mut nested.bindings {
            binding.field.insert(0, '.');
            binding.field.insert_str(0, parent);
        }
        self.bindings.extend(nested.bindings);
    }

    /// Rejects ambiguous environment mappings in a generated contract.
    pub(crate) fn validate(&self) -> Result<(), EnvironmentContractError> {
        let mut seen = BTreeMap::<&str, &str>::new();
        for binding in &self.bindings {
            if let Some(first_field) = seen.insert(&binding.variable, &binding.field) {
                return Err(EnvironmentContractError {
                    variable: binding.variable.clone(),
                    first_field: String::from(first_field),
                    second_field: binding.field.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Invalid generated environment contract.
#[derive(Debug, thiserror::Error)]
#[error(
    "environment variable `{variable}` maps to both configuration fields `{first_field}` and `{second_field}`"
)]
pub(crate) struct EnvironmentContractError {
    /// Environment variable mapped more than once.
    variable: String,
    /// First field mapped to the variable.
    first_field: String,
    /// Second field mapped to the variable.
    second_field: String,
}

impl EnvironmentContractError {
    /// Returns the ambiguous environment variable.
    pub(crate) fn variable(&self) -> &str {
        &self.variable
    }
}

/// Typed environment conversion failure returned through the derive protocol.
#[derive(Debug, thiserror::Error)]
#[error(
    "invalid value from environment variable `{variable}` for configuration field `{field}`: {source}"
)]
pub struct EnvironmentError {
    /// Rust configuration field receiving the value.
    field: String,
    /// Environment variable mapped to the field.
    variable: String,
    /// Underlying typed conversion failure.
    source: EnvironmentValueError,
}

impl EnvironmentError {
    /// Qualifies an environment conversion error with one nested parent field.
    #[doc(hidden)]
    #[must_use]
    pub fn __within(mut self, parent: &'static str) -> Self {
        self.field.insert(0, '.');
        self.field.insert_str(0, parent);
        self
    }

    /// Returns the dot-qualified configuration field path.
    pub(crate) fn field(&self) -> &str {
        &self.field
    }

    /// Returns the environment variable mapped to the field.
    pub(crate) fn variable(&self) -> &str {
        &self.variable
    }
}

/// Reads and decodes one mapped field from an environment scope.
///
/// # Errors
///
/// Returns an error when a present value is not valid UTF-8 or cannot be
/// deserialized as the requested configuration field type.
pub fn parse_environment_field<T: DeserializeOwned>(
    environment: &Environment,
    field: &'static str,
    variable: &str,
) -> Result<Option<T>, EnvironmentError> {
    let Some(value) = environment.raw(variable) else {
        return Ok(None);
    };
    let Some(value) = value.to_str() else {
        return Err(EnvironmentError {
            field: String::from(field),
            variable: String::from(variable),
            source: EnvironmentValueError::NonUtf8,
        });
    };

    T::deserialize(EnvironmentValueDeserializer { input: value }).map(Some).map_err(|source| {
        EnvironmentError { field: String::from(field), variable: String::from(variable), source }
    })
}

/// Error produced by the scalar environment deserializer.
///
/// Variants deliberately carry no downstream deserializer message. A custom
/// `Deserialize` implementation is allowed to include its input in a Serde
/// error, so retaining arbitrary messages here would make environment-backed
/// credentials observable through Argx diagnostics.
#[derive(Clone, Copy, Debug, thiserror::Error)]
enum EnvironmentValueError {
    /// A present operating-system value was not valid UTF-8.
    #[error("value is not valid UTF-8")]
    NonUtf8,
    /// The scalar could not be decoded as the requested Rust type.
    #[error("value is not valid for the configuration field type")]
    Invalid,
    /// The requested type requires structured environment syntax Argx has not defined.
    #[error("structured environment values are not supported; use a TOML layer instead")]
    Structured,
}

impl de::Error for EnvironmentValueError {
    fn custom<T: fmt::Display>(_message: T) -> Self {
        Self::Invalid
    }
}

/// Serde deserializer for one UTF-8 environment value.
///
/// Environment variables are scalar strings. Scalar Rust values are decoded
/// according to the type requested by Serde. Structured values deliberately
/// remain unsupported until Argx defines an explicit environment syntax for
/// them.
#[derive(Clone, Copy, Debug)]
struct EnvironmentValueDeserializer<'de> {
    /// Raw UTF-8 environment text.
    input: &'de str,
}

impl EnvironmentValueDeserializer<'_> {
    /// Parses the raw text as one scalar type.
    fn parse<T>(&self) -> Result<T, EnvironmentValueError>
    where
        T: FromStr,
    {
        self.input.parse::<T>().map_err(|_| EnvironmentValueError::Invalid)
    }

    /// Returns the common unsupported-structured-value error.
    const fn structured() -> EnvironmentValueError {
        EnvironmentValueError::Structured
    }
}

impl<'de> Deserializer<'de> for EnvironmentValueDeserializer<'de> {
    type Error = EnvironmentValueError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.input)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse()?)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i128(self.parse()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse()?)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u128(self.parse()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.parse()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.parse()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut chars = self.input.chars();
        let Some(value) = chars.next() else {
            return Err(EnvironmentValueError::Invalid);
        };
        if chars.next().is_some() {
            return Err(EnvironmentValueError::Invalid);
        }
        visitor.visit_char(value)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.input)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.input.to_owned())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.input.as_bytes())
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.input.as_bytes().to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.input.is_empty() {
            visitor.visit_unit()
        } else {
            Err(EnvironmentValueError::Invalid)
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Self::structured())
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Self::structured())
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Self::structured())
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Self::structured())
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Self::structured())
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(StrDeserializer::<EnvironmentValueError>::new(self.input))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit enum used to exercise Serde's string enum path.
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    enum Mode {
        /// Development mode.
        Development,
        /// Production mode.
        Production,
    }

    /// Type whose custom deserializer deliberately tries to echo its input.
    #[derive(Debug)]
    struct EchoingSecret;

    impl<'de> serde::Deserialize<'de> for EchoingSecret {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let value = <String as serde::Deserialize>::deserialize(deserializer)?;
            Err(<D::Error as de::Error>::custom(format!("rejected secret `{value}`")))
        }
    }

    /// Builds a deterministic environment scope from UTF-8 string pairs.
    fn environment(values: &[(&str, &str)]) -> Environment {
        Environment::from_utf8(
            values.iter().map(|(key, value)| (String::from(*key), String::from(*value))).collect(),
        )
    }

    #[test]
    fn scalar_environment_values_follow_the_requested_serde_type() {
        let environment = environment(&[
            ("BOOL", "true"),
            ("COUNT", "42"),
            ("TEXT", "hello"),
            ("MODE", "Production"),
        ]);

        assert_eq!(
            parse_environment_field::<bool>(&environment, "enabled", "BOOL")
                .expect("bool should parse"),
            Some(true)
        );
        assert_eq!(
            parse_environment_field::<usize>(&environment, "count", "COUNT")
                .expect("integer should parse"),
            Some(42)
        );
        assert_eq!(
            parse_environment_field::<Option<String>>(&environment, "text", "TEXT")
                .expect("optional strings should parse"),
            Some(Some(String::from("hello")))
        );
        assert_eq!(
            parse_environment_field::<Mode>(&environment, "mode", "MODE")
                .expect("unit enum should parse"),
            Some(Mode::Production)
        );
    }

    #[test]
    fn structured_environment_values_fail_explicitly() {
        let environment = environment(&[("TAGS", "one,two")]);
        let error = parse_environment_field::<Vec<String>>(&environment, "tags", "TAGS")
            .expect_err("Argx has not declared an environment collection syntax");

        assert!(error.to_string().contains("structured environment values are not supported"));
    }

    #[test]
    fn custom_deserializer_errors_cannot_echo_environment_values() {
        const SECRET: &str = "credential-that-must-not-appear";
        let environment = environment(&[("SECRET", SECRET)]);
        let error = parse_environment_field::<EchoingSecret>(&environment, "secret", "SECRET")
            .expect_err("custom deserializer should reject the value");

        assert!(!error.to_string().contains(SECRET));
        assert!(!format!("{error:?}").contains(SECRET));
        let source =
            std::error::Error::source(&error).expect("conversion error should be retained");
        assert!(!source.to_string().contains(SECRET));
        assert_eq!(source.to_string(), "value is not valid for the configuration field type");
    }
}
