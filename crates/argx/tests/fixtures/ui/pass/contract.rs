#![allow(dead_code)]

use argx::ContractType as _;

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

fn main() {
    let _ = Message::type_contract();
    let _ = Cli::type_contract();
}
