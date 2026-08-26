#![allow(dead_code)]

use argx::{ContractType as _, Parser as _};

#[derive(argx::Contract)]
struct Recursive<T, const N: usize> {
    value: T,
    children: Vec<Box<Self>>,
    fixed: [u8; N],
}

#[derive(argx::Contract)]
enum Message {
    Unit,
    Tuple(Recursive<String, 4>),
    Struct { path: std::path::PathBuf },
}

#[derive(argx::Parser, argx::Contract)]
#[argx(name = "tool")]
struct Cli {
    #[argx(long)]
    output: String,
}

#[argx::contract(Cli)]
fn cli_contract(_command: Cli) -> Result<(), ()> {
    Ok(())
}

mod private_invocation_types {
    #[derive(Debug, PartialEq, Eq, argx::Contract)]
    enum PrivateFormat {
        Json,
    }

    impl std::str::FromStr for PrivateFormat {
        type Err = &'static str;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "json" => Ok(Self::Json),
                _ => Err("expected `json`"),
            }
        }
    }

    #[derive(argx::Args)]
    struct PrivateCommon {
        #[argx(long)]
        format: PrivateFormat,
    }

    #[derive(argx::Args)]
    struct PrivateRun {
        value: PrivateFormat,
    }

    #[derive(argx::Contract)]
    struct PrivateOutput {
        accepted: bool,
    }

    #[derive(argx::Contract)]
    enum PrivateError {
        Rejected,
    }

    #[argx::contract(PrivateRun)]
    fn private_run_contract(_command: PrivateRun) -> Result<PrivateOutput, PrivateError> {
        Ok(PrivateOutput { accepted: true })
    }

    #[derive(argx::Subcommand)]
    enum PrivateCommands {
        Run(PrivateRun),
    }

    #[derive(argx::Parser)]
    pub struct PublicCli {
        #[argx(long)]
        direct: PrivateFormat,
        #[argx(flatten)]
        common: PrivateCommon,
        #[argx(subcommand)]
        command: PrivateCommands,
    }
}

pub use private_invocation_types::PublicCli as ReexportedPrivateCli;

fn main() {
    let _ = Message::type_contract();
    let _ = Cli::type_contract();
    let _ = Cli::contract(argx::ContractRequest::root());
    let _ = ReexportedPrivateCli::contract(argx::ContractRequest::root().recursive());
}
