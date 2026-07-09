//! M5 checkpoint: the full custody loop against the real batch_n4 fixture —
//! two SEP-41 deposits escrow tokens and enqueue L2 credits, the proven batch
//! consumes them (1 transfer + 1 withdrawal on L2), and the withdrawal pays
//! out on L1. Fixture scenario: `cargo run -p harness -- demo-batch`
//! (fixtures/batch_n4/meta.json records the constants replayed here).

use rollup::{BatchEnvelope, RollupContract, RollupContractClient, Withdrawal};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, vec, Address, Bytes, BytesN, Env, String as SString, Vec};

const VK: &[u8] = include_bytes!("../../../fixtures/batch_n4/vk.bin");
const PROOF: &[u8] = include_bytes!("../../../fixtures/batch_n4/proof");
const PUBLIC_INPUTS: &[u8] = include_bytes!("../../../fixtures/batch_n4/public_inputs");
const META: &str = include_str!("../../../fixtures/batch_n4/meta.json");

struct Meta {
    old_root: [u8; 32],
    new_root: [u8; 32],
    da_commitment: [u8; 32],
    alice_pk_x: [u8; 32],
    bob_pk_x: [u8; 32],
    wd_dest: String,
}

fn hex32(s: &str) -> [u8; 32] {
    let s = s.trim_start_matches("0x");
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

fn meta() -> Meta {
    let v: serde_json::Value = serde_json::from_str(META).unwrap();
    Meta {
        old_root: hex32(v["old_root"].as_str().unwrap()),
        new_root: hex32(v["new_root"].as_str().unwrap()),
        da_commitment: hex32(v["da_commitment"].as_str().unwrap()),
        alice_pk_x: hex32(v["deposits"][0]["pk_x"].as_str().unwrap()),
        bob_pk_x: hex32(v["deposits"][1]["pk_x"].as_str().unwrap()),
        wd_dest: v["withdrawals"][0]["dest"].as_str().unwrap().to_string(),
    }
}

struct Setup<'a> {
    env: Env,
    rollup: RollupContractClient<'a>,
    token: token::TokenClient<'a>,
    alice_l1: Address,
    bob_l1: Address,
    meta: Meta,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();
    let m = meta();

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(&env, &sac.address());
    let token_client = token::TokenClient::new(&env, &sac.address());

    let alice_l1 = Address::generate(&env);
    let bob_l1 = Address::generate(&env);
    token_admin.mint(&alice_l1, &10_000);
    token_admin.mint(&bob_l1, &10_000);

    let vk = Bytes::from_slice(&env, VK);
    let genesis = BytesN::from_array(&env, &m.old_root);
    let rollup_id = env.register(RollupContract, (sac.address(), vk, genesis));
    let rollup = RollupContractClient::new(&env, &rollup_id);

    Setup { env: env.clone(), rollup, token: token_client, alice_l1, bob_l1, meta: m }
}

fn fixture_envelope(env: &Env, m: &Meta) -> BatchEnvelope {
    let wd_dest = Address::from_string(&SString::from_str(env, &m.wd_dest));
    BatchEnvelope {
        new_root: BytesN::from_array(env, &m.new_root),
        deposit_count: 2,
        withdrawals: vec![env, Withdrawal { dest: wd_dest, amount: 100 }],
        da_commitment: BytesN::from_array(env, &m.da_commitment),
        proof: Bytes::from_slice(env, PROOF),
    }
}

fn do_deposits(s: &Setup) {
    let alice_pk = BytesN::from_array(&s.env, &s.meta.alice_pk_x);
    let bob_pk = BytesN::from_array(&s.env, &s.meta.bob_pk_x);
    s.rollup.deposit(&s.alice_l1, &alice_pk, &1000);
    s.rollup.deposit(&s.bob_l1, &bob_pk, &500);
}

#[test]
fn full_custody_loop() {
    let s = setup();
    let rollup_addr = s.rollup.address.clone();

    do_deposits(&s);
    assert_eq!(s.token.balance(&rollup_addr), 1500);
    assert_eq!(s.token.balance(&s.alice_l1), 9000);
    assert_eq!(s.rollup.pending_deposit_count(), 2);

    // Sanity: the contract's PI assembly must reproduce the prover's blob.
    let envelope = fixture_envelope(&s.env, &s.meta);
    let sequencer = Address::generate(&s.env);
    s.env.cost_estimate().budget().reset_unlimited();
    s.rollup.submit_batch(&sequencer, &envelope);
    println!(
        "submit_batch budget: cpu={} mem={}",
        s.env.cost_estimate().budget().cpu_instruction_cost(),
        s.env.cost_estimate().budget().memory_bytes_cost()
    );

    // Escrow: 1500 in, 100 withdrawn.
    assert_eq!(s.token.balance(&rollup_addr), 1400);
    let wd_dest = Address::from_string(&SString::from_str(&s.env, &s.meta.wd_dest));
    assert_eq!(s.token.balance(&wd_dest), 100);

    assert_eq!(s.rollup.root(), BytesN::from_array(&s.env, &s.meta.new_root));
    assert_eq!(s.rollup.batch_num(), 1);
    assert_eq!(s.rollup.pending_deposit_count(), 0);

    // Replay must fail: the root has advanced.
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn fixture_public_inputs_match_contract_assembly() {
    // The checked-in bb public_inputs blob must equal
    // old_root || new_root || deposit_hash || withdraw_hash as the contract
    // computes them; full_custody_loop already proves this transitively
    // (verification would fail otherwise), but check the roots explicitly.
    let m = meta();
    assert_eq!(&PUBLIC_INPUTS[..32], &m.old_root);
    assert_eq!(&PUBLIC_INPUTS[32..64], &m.new_root);
}

#[test]
fn tampered_da_commitment_fails() {
    // A sequencer claiming a different DA blob than the one proven must be
    // rejected — this is the validium's data-binding guarantee.
    let s = setup();
    do_deposits(&s);
    let mut envelope = fixture_envelope(&s.env, &s.meta);
    let mut tampered = s.meta.da_commitment;
    tampered[31] ^= 0x01;
    envelope.da_commitment = BytesN::from_array(&s.env, &tampered);
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn wrong_new_root_fails() {
    let s = setup();
    do_deposits(&s);
    let mut envelope = fixture_envelope(&s.env, &s.meta);
    let mut tampered = s.meta.new_root;
    tampered[31] ^= 0x01;
    envelope.new_root = BytesN::from_array(&s.env, &tampered);
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn tampered_withdrawal_amount_fails() {
    let s = setup();
    do_deposits(&s);
    let mut envelope = fixture_envelope(&s.env, &s.meta);
    let wd = envelope.withdrawals.get(0).unwrap();
    envelope.withdrawals = vec![&s.env, Withdrawal { dest: wd.dest, amount: 150 }];
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn redirected_withdrawal_fails() {
    let s = setup();
    do_deposits(&s);
    let mut envelope = fixture_envelope(&s.env, &s.meta);
    // Attacker redirects the payout to their own address.
    envelope.withdrawals =
        vec![&s.env, Withdrawal { dest: Address::generate(&s.env), amount: 100 }];
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn wrong_deposit_count_fails() {
    let s = setup();
    do_deposits(&s);
    let mut envelope = fixture_envelope(&s.env, &s.meta);
    envelope.deposit_count = 1;
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());

    envelope.deposit_count = 3; // more than the queue holds
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn missing_deposits_fail() {
    // Same proof without the on-chain deposits: queue prefix hash differs.
    let s = setup();
    let envelope = fixture_envelope(&s.env, &s.meta);
    let sequencer = Address::generate(&s.env);
    assert!(s.rollup.try_submit_batch(&sequencer, &envelope).is_err());
}

#[test]
fn deposit_validation() {
    let s = setup();
    let pk = BytesN::from_array(&s.env, &s.meta.alice_pk_x);
    // Zero, negative, and >= 2^64 amounts are rejected.
    assert!(s.rollup.try_deposit(&s.alice_l1, &pk, &0).is_err());
    assert!(s.rollup.try_deposit(&s.alice_l1, &pk, &-5).is_err());
    assert!(s.rollup.try_deposit(&s.alice_l1, &pk, &(i128::from(u64::MAX) + 1)).is_err());
    // Non-canonical pk_x (>= r) and zero pk_x are rejected.
    let non_canonical = BytesN::from_array(&s.env, &[0xffu8; 32]);
    assert!(s.rollup.try_deposit(&s.alice_l1, &non_canonical, &100).is_err());
    let zero = BytesN::from_array(&s.env, &[0u8; 32]);
    assert!(s.rollup.try_deposit(&s.alice_l1, &zero, &100).is_err());
    // Public padding keypair (sk=7) must never receive credits.
    let pad = BytesN::from_array(&s.env, &rollup::PAD_PK_X);
    assert!(s.rollup.try_deposit(&s.alice_l1, &pad, &100).is_err());
}

#[test]
fn deposit_lifetime_credit_cap() {
    // Two deposits that sum past u64::MAX for the same pk_x must reject the
    // second (prevents an unprovable queue head / permanent jam). Dedicated
    // setup so alice can be minted near u64::MAX of the custody asset.
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();
    let m = meta();
    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(&env, &sac.address());
    let alice = Address::generate(&env);
    token_admin.mint(&alice, &i128::from(u64::MAX));
    let vk = Bytes::from_slice(&env, VK);
    let genesis = BytesN::from_array(&env, &m.old_root);
    let rollup_id = env.register(RollupContract, (sac.address(), vk, genesis));
    let rollup = RollupContractClient::new(&env, &rollup_id);
    let pk = BytesN::from_array(&env, &m.alice_pk_x);

    let almost = i128::from(u64::MAX - 10);
    rollup.deposit(&alice, &pk, &almost);
    assert!(rollup.try_deposit(&alice, &pk, &20).is_err());
    // Remaining room is 10; a 5-unit top-up still fits the lifetime cap.
    rollup.deposit(&alice, &pk, &5);
}
