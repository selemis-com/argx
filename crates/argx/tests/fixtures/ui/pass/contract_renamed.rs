#![allow(dead_code)]

use cli_args::ContractType as _;

#[derive(cli_args::Contract)]
struct Payload {
    value: String,
}

fn main() {
    let _ = Payload::type_contract();
}
