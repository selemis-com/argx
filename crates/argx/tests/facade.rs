//! Public facade wiring tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    /// Minimal parser used to verify that the derive and trait share the facade name.
    #[derive(argx::Parser)]
    struct Cli;

    /// Requires the public parser trait implemented by the derive.
    const fn assert_parser<T: argx::Parser>() {}

    #[test]
    fn parser_derive_is_reexported_by_the_facade() {
        assert_parser::<Cli>();
    }
}
