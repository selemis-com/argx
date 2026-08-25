<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg">
  <img alt="Argx" src="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg" width="100%" height="140px">
</picture>

<p align="center">
  Expressive command line argument parser for Rust
</p>

<br/>

<p align="center">
  <a href="https://crates.io/crates/argx"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/v/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/v/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="Version"></picture></a>
  <a href="#license"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/l/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/l/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="MIT OR Apache-2.0"></picture></a>
</p>

## MSRV

<!--
When updating this, also update:
- Cargo.toml
- .github/workflows/ci.yml
-->

The current MSRV (minimum supported Rust version) is 1.95.

Argx will keep a rolling MSRV policy of **at least** two versions behind the
latest stable release (so if the latest stable release is 1.97, we would
support 1.95).

Note that the MSRV is not increased automatically.

## Contributing

Contributions to Argx are welcome. See the [Contributing Guide](CONTRIBUTING.md) for information on reporting bugs, proposing features, submitting pull requests, and the licensing terms that apply to contributions.

## Security Policy

If you believe you have found a security vulnerability, please do not report it through GitHub Issues. See our [Security Policy](SECURITY.md) for reporting instructions.

## Credit

Argx is inspired in part by [Usage](https://github.com/jdx/usage), [Clap](https://github.com/clap-rs/clap) and [Incur](https://github.com/wevm/incur).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

This software includes third-party components subject to separate license
terms. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Argx by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
