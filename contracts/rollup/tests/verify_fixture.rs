//! M1 checkpoint: a real UltraHonk proof (nargo 1.0.0-beta.11 + bb 0.87.0,
//! keccak oracle) verifies inside a Soroban contract, and a tampered proof is
//! rejected. Uses the checked-in fixtures so this runs without nargo/bb
//! installed; regenerate with `just prove trivial`.

use rollup::{RollupContract, RollupContractClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env};

const VK: &[u8] = include_bytes!("../../../fixtures/trivial/vk.bin");
const PROOF: &[u8] = include_bytes!("../../../fixtures/trivial/proof");
const PUBLIC_INPUTS: &[u8] = include_bytes!("../../../fixtures/trivial/public_inputs");

fn setup(env: &Env) -> RollupContractClient<'_> {
    let vk = Bytes::from_slice(env, VK);
    let token = Address::generate(env);
    let genesis = BytesN::from_array(env, &[0u8; 32]);
    let id = env.register(RollupContract, (token, vk, genesis));
    RollupContractClient::new(env, &id)
}

#[test]
fn real_proof_verifies() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    let proof = Bytes::from_slice(&env, PROOF);
    let pis = Bytes::from_slice(&env, PUBLIC_INPUTS);

    env.cost_estimate().budget().reset_unlimited();
    client.verify(&pis, &proof);

    println!(
        "verify budget: cpu={} mem={}",
        env.cost_estimate().budget().cpu_instruction_cost(),
        env.cost_estimate().budget().memory_bytes_cost()
    );
}

#[test]
fn tampered_proof_fails() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    let mut tampered = PROOF.to_vec();
    tampered[100] ^= 0x01;
    let proof = Bytes::from_slice(&env, &tampered);
    let pis = Bytes::from_slice(&env, PUBLIC_INPUTS);

    assert!(client.try_verify(&pis, &proof).is_err());
}

#[test]
fn wrong_public_input_fails() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    let proof = Bytes::from_slice(&env, PROOF);
    let mut wrong = PUBLIC_INPUTS.to_vec();
    wrong[31] ^= 0x01;
    let pis = Bytes::from_slice(&env, &wrong);

    assert!(client.try_verify(&pis, &proof).is_err());
}

#[test]
fn wrong_proof_length_fails() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let client = setup(&env);

    let proof = Bytes::from_slice(&env, &PROOF[..PROOF.len() - 1]);
    let pis = Bytes::from_slice(&env, PUBLIC_INPUTS);

    assert!(client.try_verify(&pis, &proof).is_err());
}
