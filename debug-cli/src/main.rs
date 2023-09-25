use std::vec;

/// Testing cli

fn main() {
    let contract = utils::substrate::SubstrateContract::from_account(
        "fix enable minimum debate purse act congress poet give alley inch town".to_string(), // sample seed, NEVER expose it in clear
        None,
    )
    .unwrap();

    // Prepare for dummy phala call
    let nonce = [1; 32];
    let value = contract
        .instance
        .call_msg("get", vec![], Some(nonce))
        .unwrap();
    println!("{}", value);
}
