//! M4 checkpoint: the real batch_n4 proof (2 deposits, 1 transfer,
//! 1 withdrawal, 2 padding entries) verifies on-chain. Regenerate fixtures
//! with `cargo run -p harness -- demo-batch`.

use rollup::{RollupContract, RollupContractClient};
use soroban_sdk::{Bytes, Env};

const VK: &[u8] = include_bytes!("../../../fixtures/batch_n4/vk.bin");
const PROOF: &[u8] = include_bytes!("../../../fixtures/batch_n4/proof");
const PUBLIC_INPUTS: &[u8] = include_bytes!("../../../fixtures/batch_n4/public_inputs");

fn setup(env: &Env) -> RollupContractClient<'_> {
    let vk = Bytes::from_slice(env, VK);
    let id = env.register(RollupContract, (vk,));
    RollupContractClient::new(env, &id)
}

#[test]
fn batch_proof_verifies() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    assert_eq!(PUBLIC_INPUTS.len(), 128, "4 public inputs expected");
    let proof = Bytes::from_slice(&env, PROOF);
    let pis = Bytes::from_slice(&env, PUBLIC_INPUTS);

    env.cost_estimate().budget().reset_unlimited();
    client.verify(&pis, &proof);

    println!(
        "batch_n4 verify budget: cpu={} mem={}",
        env.cost_estimate().budget().cpu_instruction_cost(),
        env.cost_estimate().budget().memory_bytes_cost()
    );
}

#[test]
fn batch_proof_rejects_wrong_root() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    // Flip a byte in new_root (second 32-byte word).
    let mut wrong = PUBLIC_INPUTS.to_vec();
    wrong[63] ^= 0x01;
    let proof = Bytes::from_slice(&env, PROOF);
    let pis = Bytes::from_slice(&env, &wrong);
    assert!(client.try_verify(&pis, &proof).is_err());
}
