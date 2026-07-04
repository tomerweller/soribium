use harness::poseidon::{fr_from_u64, to_hex, Hasher};
use harness::tree::{Account, Tree};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        // Shared test vectors pinned in circuits/lib tests (M2 checkpoint).
        "vectors" => vectors(),
        // Schnorr vectors pinned in circuits/lib tests (M3 checkpoint).
        "sig-vectors" => sig_vectors(),
        // Deterministic demo batch -> Prover.toml -> bb -> fixtures/batch_n4.
        "demo-batch" => demo_batch(),
        _ => {
            eprintln!("usage: harness vectors");
            std::process::exit(2);
        }
    }
}

fn vectors() {
    let hasher = Hasher::new();

    println!(
        "hash2(1, 2)            = {}",
        to_hex(&hasher.hash2(fr_from_u64(1), fr_from_u64(2)))
    );
    println!(
        "hash4(1, 2, 3, 4)      = {}",
        to_hex(&hasher.hash(&[fr_from_u64(1), fr_from_u64(2), fr_from_u64(3), fr_from_u64(4)]))
    );

    let empty = Tree::new();
    println!("empty_root (depth 8)   = {}", to_hex(&empty.root(&hasher)));

    let account = Account {
        pk_x: fr_from_u64(1234),
        balance: 100,
        nonce: 0,
    };
    println!(
        "leaf(1234, 100, 0)     = {}",
        to_hex(&Tree::leaf_value(&hasher, Some(&account)))
    );

    let mut one = Tree::new();
    one.set(5, account);
    println!("root(leaf@5)           = {}", to_hex(&one.root(&hasher)));
}

fn sig_vectors() {
    use ark_ec::AffineRepr;
    use harness::keys::{coord_to_fr, sign_with_nonce, verify, Keypair};

    let hasher = Hasher::new();
    let gen = ark_grumpkin::Affine::generator();
    println!("gen_x   = {}", to_hex(&coord_to_fr(&gen.x)));
    println!("gen_y   = {}", to_hex(&coord_to_fr(&gen.y)));

    // Deterministic vector: sk = 7, k = 13, msg = 42.
    let keypair = Keypair::from_sk(ark_grumpkin::Fr::from(7u64));
    println!("pk_x    = {}", to_hex(&keypair.pk_x()));
    println!("pk_y    = {}", to_hex(&keypair.pk_y()));

    let msg = fr_from_u64(42);
    let sig = sign_with_nonce(&hasher, &keypair, msg, ark_grumpkin::Fr::from(13u64));
    assert!(verify(&hasher, &keypair.pk, msg, &sig), "self-check failed");
    let (s_lo, s_hi) = sig.s_limbs();
    println!("msg     = 42");
    println!("r_x     = {}", to_hex(&sig.r_x));
    println!("r_y     = {}", to_hex(&sig.r_y));
    println!("s_lo    = {}", to_hex(&s_lo));
    println!("s_hi    = {}", to_hex(&s_hi));
}

/// The deterministic scenario replayed by the contract's custody-loop test
/// (contracts/rollup/tests/custody_loop.rs). Every constant here is part of
/// the fixture contract between harness and test: alice sk=101 deposits 1000,
/// bob sk=202 deposits 500, alice pays bob 200, bob withdraws 100 to the
/// contract-type address derived from [7u8; 32] (C-addresses receive SAC
/// tokens without a trustline, so the test can assert its balance directly).
fn wd_addr() -> String {
    stellar_strkey::Contract([7u8; 32]).to_string()
}

fn demo_batch() {
    use harness::batch::{build_batch, DepositRequest, TxRequest};
    use harness::l1::address_to_field;
    use harness::prover;
    use harness::tree::Tree;
    use rand::SeedableRng;

    let hasher = Hasher::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);

    let alice = harness::keys::Keypair::from_sk(ark_grumpkin::Fr::from(101u64));
    let bob = harness::keys::Keypair::from_sk(ark_grumpkin::Fr::from(202u64));
    let wd_addr = wd_addr();
    let wd_field = address_to_field(&hasher, &wd_addr);

    let mut tree = Tree::new();
    let witness = build_batch(
        &hasher,
        &mut tree,
        2,
        4,
        &[
            DepositRequest { pk_x: alice.pk_x(), amount: 1000 },
            DepositRequest { pk_x: bob.pk_x(), amount: 500 },
        ],
        &[
            TxRequest { from: &alice, to_field: bob.pk_x(), amount: 200, is_withdraw: false },
            TxRequest { from: &bob, to_field: wd_field, amount: 100, is_withdraw: true },
        ],
        &mut rng,
    );

    println!("old_root      = {}", to_hex(&witness.old_root));
    println!("new_root      = {}", to_hex(&witness.new_root));
    println!("deposit_hash  = {}", to_hex(&witness.deposit_hash));
    println!("withdraw_hash = {}", to_hex(&witness.withdraw_hash));

    // Metadata for the contract test to replay the same scenario.
    let meta = serde_json::json!({
        "old_root": to_hex(&witness.old_root),
        "new_root": to_hex(&witness.new_root),
        "deposit_hash": to_hex(&witness.deposit_hash),
        "withdraw_hash": to_hex(&witness.withdraw_hash),
        "deposits": [
            { "pk_x": to_hex(&alice.pk_x()), "amount": 1000 },
            { "pk_x": to_hex(&bob.pk_x()), "amount": 500 },
        ],
        "withdrawals": [ { "dest": wd_addr, "amount": 100 } ],
    });
    let root = prover::repo_root();
    let fixture_dir = root.join("fixtures/batch_n4");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    std::fs::write(
        fixture_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();

    let toml = prover::to_prover_toml(&witness);
    prover::prove("batch_n4", &toml).expect("prove pipeline failed");
}
