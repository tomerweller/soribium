//! Direct equivalence of the contract's own Poseidon fold + address encoding
//! (publics.rs, via soroban host functions) against the harness-generated
//! golden vectors in fixtures/vectors.json — the exact values the circuits
//! and the wallet pin. Before this test, publics.rs agreed with the other
//! stacks only transitively through one checked-in proof.
#![cfg(test)]

use rollup::publics::{address_to_field, fold, DOMAIN_DEP, DOMAIN_WD};
use soroban_sdk::{Address, BytesN, Env, String as SString};

fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../fixtures/vectors.json")).unwrap()
}

fn word(env: &Env, hex0x: &str) -> BytesN<32> {
    let hex = hex0x.strip_prefix("0x").unwrap();
    let mut arr = [0u8; 32];
    for i in 0..32 {
        arr[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    BytesN::from_array(env, &arr)
}

#[test]
fn deposit_fold_chain_matches_harness() {
    let env = Env::default();
    let v = vectors();
    let mut acc = BytesN::from_array(&env, &[0u8; 32]);
    for d in v["demo"]["deposits"].as_array().unwrap() {
        let pk = word(&env, d["pk_x"].as_str().unwrap());
        let amount: i128 = d["amount"].as_str().unwrap().parse().unwrap();
        acc = fold(&env, DOMAIN_DEP, &acc, &pk, amount);
    }
    assert_eq!(acc, word(&env, v["demo"]["deposit_hash"].as_str().unwrap()));
}

#[test]
fn withdraw_fold_and_address_encoding_match_harness() {
    let env = Env::default();
    let v = vectors();
    let dest = Address::from_string(&SString::from_str(
        &env,
        v["wd_dest"].as_str().unwrap(),
    ));

    // address_to_field agrees with harness/src/l1.rs byte-for-byte.
    let dest_field = address_to_field(&env, &dest);
    assert_eq!(dest_field, word(&env, v["wd_dest_field"].as_str().unwrap()));

    // Withdraw fold chain reproduces the proven withdraw_hash.
    let mut acc = BytesN::from_array(&env, &[0u8; 32]);
    for w in v["demo"]["withdrawals"].as_array().unwrap() {
        let amount: i128 = w["amount"].as_str().unwrap().parse().unwrap();
        acc = fold(&env, DOMAIN_WD, &acc, &dest_field, amount);
    }
    assert_eq!(acc, word(&env, v["demo"]["withdraw_hash"].as_str().unwrap()));
}
