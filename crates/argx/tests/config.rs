//! Ordered configuration layer tests.

use std::{fs, path::PathBuf};

#[cfg(feature = "toml")]
use argx::Toml;
use argx::{Argv, Defaults, Dotenv};

#[derive(Debug, argx::Config)]
struct AppConfig {
    /// Worker count.
    #[argx(long, default = 4)]
    workers: usize,

    /// Whether the feature is enabled.
    #[argx(long, default = true)]
    enabled: bool,

    /// Value required after all layers are resolved.
    #[argx(long)]
    endpoint: String,
}

fn temp_file(name: &str, extension: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "argx-config-{name}-{}-{}.{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test"),
        extension,
    ));
    fs::write(&path, contents).expect("write temporary configuration");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "toml")]
    #[test]
    fn layer_order_defines_precedence() {
        let path = temp_file("precedence", "toml", "workers = 8\nendpoint = \"from-toml\"\n");

        let toml_then_argv = AppConfig::loader()
            .layer(Defaults)
            .layer(Toml::new(&path))
            .layer(Argv::new(["app", "--workers", "16", "--endpoint", "from-cli"]))
            .resolve()
            .expect("resolve configuration");
        assert_eq!(toml_then_argv.workers, 16);
        assert_eq!(toml_then_argv.endpoint, "from-cli");
        assert!(toml_then_argv.enabled);

        let argv_then_toml = AppConfig::loader()
            .layer(Defaults)
            .layer(Argv::new(["app", "--workers", "16", "--endpoint", "from-cli"]))
            .layer(Toml::new(&path))
            .resolve()
            .expect("resolve reordered configuration");
        assert_eq!(argv_then_toml.workers, 8);
        assert_eq!(argv_then_toml.endpoint, "from-toml");

        let _ = fs::remove_file(path);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn absent_values_do_not_mask_earlier_layers() {
        let path = temp_file("sparse", "toml", "workers = 8\nendpoint = \"from-toml\"\n");

        let config = AppConfig::loader()
            .layer(Defaults)
            .layer(Toml::new(&path))
            .layer(Argv::new(["app"]))
            .resolve()
            .expect("resolve configuration");

        assert_eq!(config.workers, 8);
        assert_eq!(config.endpoint, "from-toml");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn boolean_cli_values_can_override_both_directions() {
        let config = AppConfig::loader()
            .layer(Defaults)
            .layer(Argv::new(["app", "--enabled", "false", "--endpoint", "http://localhost"]))
            .resolve()
            .expect("resolve configuration");

        assert!(!config.enabled);
    }

    #[derive(Debug, argx::Config)]
    #[argx(prefix = "ARGX_LAYER_TEST")]
    struct EnvironmentConfig {
        endpoint: String,
    }

    #[test]
    fn environment_files_are_explicit_layers() {
        let path = temp_file("environment", "env", "ARGX_LAYER_TEST_ENDPOINT=from-env-file\n");

        let config = EnvironmentConfig::loader()
            .layer(Dotenv::new(&path))
            .resolve()
            .expect("resolve environment file");

        assert_eq!(config.endpoint, "from-env-file");
        let _ = fs::remove_file(path);
    }

    #[derive(Debug, argx::Config)]
    struct NestedConfig {
        #[argx(flatten)]
        server: ServerConfig,
    }

    #[derive(Debug, argx::Config)]
    struct ServerConfig {
        #[argx(long, default = 4)]
        workers: usize,
    }

    #[test]
    fn flattened_defaults_require_a_defaults_layer() {
        let error = NestedConfig::loader()
            .resolve()
            .expect_err("nested defaults must not apply implicitly");

        let argx::ConfigError::Source(error) = error else {
            panic!("expected source error");
        };
        assert_eq!(error.field(), Some("server.workers"));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn flatten_composes_nested_layers() {
        let path = temp_file("nested", "toml", "[server]\nworkers = 8\n");
        let config = NestedConfig::loader()
            .layer(Defaults)
            .layer(Toml::new(&path))
            .layer(Argv::new(["app", "--workers", "12"]))
            .resolve()
            .expect("resolve nested configuration");

        assert_eq!(config.server.workers, 12);
        let _ = fs::remove_file(path);
    }

    #[derive(Debug, argx::Config)]
    struct InterpolatedConfig {
        endpoint: String,
    }

    #[cfg(feature = "toml")]
    #[test]
    fn interpolation_observes_only_earlier_environment_layers() {
        let env = temp_file("interpolation", "env", "ARGX_LAYER_HOST=example.invalid\n");
        let toml =
            temp_file("interpolation", "toml", "endpoint = \"https://${ARGX_LAYER_HOST}\"\n");

        let config = InterpolatedConfig::loader()
            .layer(Dotenv::new(&env))
            .layer(Toml::new(&toml))
            .resolve()
            .expect("resolve interpolated TOML");

        assert_eq!(config.endpoint, "https://example.invalid");
        let _ = fs::remove_file(env);
        let _ = fs::remove_file(toml);
    }
}
