//! Public facade wiring tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    /// Minimal parser used to verify that the derive and trait share the facade name.
    #[derive(argx::Parser)]
    struct Cli;

    /// Minimal reusable group used to verify that the derive and trait share the facade name.
    #[derive(argx::Args)]
    struct Common;

    /// Requires the public parser trait implemented by the derive.
    const fn assert_parser<T: argx::Parser>() {}

    /// Requires the public argument-group trait implemented by the derive.
    const fn assert_args<T: argx::Args>() {}

    #[test]
    fn derive_traits_are_reexported_by_the_facade() {
        assert_parser::<Cli>();
        assert_args::<Common>();
    }
}
