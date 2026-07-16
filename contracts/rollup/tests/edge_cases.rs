//! submit_batch edge cases the custody-loop suite doesn't reach:
//! withdrawal-list bounds, multi-batch FIFO queue progression, and the
//! (intentional) permissionless-submit property. All envelope validation
//! under test happens before proof verification, so a fixture-length proof
//! is enough for the rejects; the queue-progression test lands the real
//! fixture proof.
use rollup::{BatchEnvelope, RollupContract, RollupContractClient, RollupError, Withdrawal};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, vec, Address, Bytes, BytesN, Env, String as SString, Vec};

const VK: &[u8] = include_bytes!("../../../fixtures/batch_n4/vk.bin");
const PROOF: &[u8] = include_bytes!("../../../fixtures/batch_n4/proof");
const META: &str = include_str!("../../../fixtures/batch_n4/meta.json");

fn hex32(s: &str) -> [u8; 32] {
    let s = s.trim_start_matches("0x");
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

struct Setup<'a> {
    env: Env,
    rollup: RollupContractClient<'a>,
    funder: Address,
    meta: serde_json::Value,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();
    let meta: serde_json::Value = serde_json::from_str(META).unwrap();

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = token::StellarAssetClient::new(&env, &sac.address());
    let funder = Address::generate(&env);
    token_admin.mint(&funder, &1_000_000);

    let vk = Bytes::from_slice(&env, VK);
    let genesis = BytesN::from_array(&env, &hex32(meta["old_root"].as_str().unwrap()));
    let rollup_id = env.register(RollupContract, (sac.address(), vk, genesis));
    let rollup = RollupContractClient::new(&env, &rollup_id);
    Setup { env: env.clone(), rollup, funder, meta }
}

fn envelope_with_withdrawals(env: &Env, wds: Vec<Withdrawal>) -> BatchEnvelope {
    BatchEnvelope {
        new_root: BytesN::from_array(env, &[9u8; 32]),
        deposit_count: 0,
        withdrawals: wds,
        da_commitment: BytesN::from_array(env, &[0u8; 32]),
        proof: Bytes::from_slice(env, PROOF), // right length; never reaches verify
    }
}

#[test]
fn nine_withdrawals_rejected() {
    let s = setup();
    let mut wds = vec![&s.env];
    for _ in 0..9 {
        wds.push_back(Withdrawal { dest: Address::generate(&s.env), amount: 1 });
    }
    let r = s.rollup.try_submit_batch(&Address::generate(&s.env), &envelope_with_withdrawals(&s.env, wds));
    assert_eq!(r, Err(Ok(RollupError::TooManyWithdrawals)));
    // Exactly 8 passes the bound (and then fails later, at verification).
    let mut wds = vec![&s.env];
    for _ in 0..8 {
        wds.push_back(Withdrawal { dest: Address::generate(&s.env), amount: 1 });
    }
    let r = s.rollup.try_submit_batch(&Address::generate(&s.env), &envelope_with_withdrawals(&s.env, wds));
    assert_eq!(r, Err(Ok(RollupError::VerificationFailed)));
}

#[test]
fn withdrawal_amount_bounds() {
    let s = setup();
    for bad in [0i128, -5, (u64::MAX as i128) + 1] {
        let wds = vec![&s.env, Withdrawal { dest: Address::generate(&s.env), amount: bad }];
        let r = s.rollup.try_submit_batch(&Address::generate(&s.env), &envelope_with_withdrawals(&s.env, wds));
        assert_eq!(r, Err(Ok(RollupError::InvalidAmount)), "amount {bad} must be rejected");
    }
}

/// The FIFO queue advances by exactly deposit_count: entries beyond the
/// consumed prefix stay pending (with their seq/order intact) for the next
/// batch, and consumed entries are gone.
#[test]
fn partial_queue_consumption_across_batches() {
    let s = setup();
    let alice_pk = BytesN::from_array(&s.env, &hex32(s.meta["deposits"][0]["pk_x"].as_str().unwrap()));
    let bob_pk = BytesN::from_array(&s.env, &hex32(s.meta["deposits"][1]["pk_x"].as_str().unwrap()));
    let carol_pk = BytesN::from_array(&s.env, &{
        let mut a = [0u8; 32];
        a[31] = 9; // canonical, nonzero, not PAD
        a
    });

    // Queue three; the fixture batch consumes the first two.
    s.rollup.deposit(&s.funder, &alice_pk, &1000);
    s.rollup.deposit(&s.funder, &bob_pk, &500);
    s.rollup.deposit(&s.funder, &carol_pk, &700);
    assert_eq!((s.rollup.dep_head(), s.rollup.dep_tail()), (0, 3));

    let wd_dest = Address::from_string(&SString::from_str(&s.env, s.meta["withdrawals"][0]["dest"].as_str().unwrap()));
    let envelope = BatchEnvelope {
        new_root: BytesN::from_array(&s.env, &hex32(s.meta["new_root"].as_str().unwrap())),
        deposit_count: 2,
        withdrawals: vec![&s.env, Withdrawal { dest: wd_dest, amount: 100 }],
        da_commitment: BytesN::from_array(&s.env, &hex32(s.meta["da_commitment"].as_str().unwrap())),
        proof: Bytes::from_slice(&s.env, PROOF),
    };
    s.env.cost_estimate().budget().reset_unlimited();
    s.rollup.submit_batch(&Address::generate(&s.env), &envelope);

    // Head advanced past the consumed prefix; carol still queued with seq 2.
    assert_eq!((s.rollup.dep_head(), s.rollup.dep_tail()), (2, 3));
    assert_eq!(s.rollup.pending_deposit_count(), 1);
    assert_eq!(s.rollup.get_pending_deposit(&2).amount, 700);
    // Consumed entries are released from storage entirely.
    assert!(s.rollup.try_get_pending_deposit(&0).is_err());
    assert!(s.rollup.try_get_pending_deposit(&1).is_err());
    assert_eq!(s.rollup.batch_num(), 1);
}

/// Submission is intentionally permissionless: the proof binds old_root and
/// the contract recomputes the deposit/withdraw folds itself, so a valid
/// envelope is valid no matter who relays it. This test documents that as a
/// design decision (any address, no registered-sequencer check).
#[test]
fn submit_is_permissionless_by_design() {
    let s = setup();
    let alice_pk = BytesN::from_array(&s.env, &hex32(s.meta["deposits"][0]["pk_x"].as_str().unwrap()));
    let bob_pk = BytesN::from_array(&s.env, &hex32(s.meta["deposits"][1]["pk_x"].as_str().unwrap()));
    s.rollup.deposit(&s.funder, &alice_pk, &1000);
    s.rollup.deposit(&s.funder, &bob_pk, &500);

    let wd_dest = Address::from_string(&SString::from_str(&s.env, s.meta["withdrawals"][0]["dest"].as_str().unwrap()));
    let envelope = BatchEnvelope {
        new_root: BytesN::from_array(&s.env, &hex32(s.meta["new_root"].as_str().unwrap())),
        deposit_count: 2,
        withdrawals: vec![&s.env, Withdrawal { dest: wd_dest, amount: 100 }],
        da_commitment: BytesN::from_array(&s.env, &hex32(s.meta["da_commitment"].as_str().unwrap())),
        proof: Bytes::from_slice(&s.env, PROOF),
    };
    let random_third_party = Address::generate(&s.env);
    s.env.cost_estimate().budget().reset_unlimited();
    s.rollup.submit_batch(&random_third_party, &envelope);
    assert_eq!(s.rollup.batch_num(), 1);
}
