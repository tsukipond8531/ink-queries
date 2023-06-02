use std::vec;

/// Testing cli

fn main() {
    let contract = utils::substrate::SubstrateContract::from_account(
        "fix enable minimum debate purse act congress poet give alley inch town".to_string(),
        None,
    )
    .unwrap();

    let value = contract
        .instance
        .call_msg("get", vec!["2".to_string()])
        .unwrap();
    let value = value.data;
    println!("{}", value);
}