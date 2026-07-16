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
        // Larger batches for cost-scaling measurements -> fixtures/batch_nN.
        "demo-batch-n16" => demo_batch_sized(4, 16, "batch_n16"),
        "demo-batch-n64" => demo_batch_sized(8, 64, "batch_n64"),
        "demo-batch-n128" => demo_batch_sized(8, 128, "batch_n128"),
        "demo-batch-n256" => demo_batch_sized(8, 256, "batch_n256"),
        // Witness/signature vectors for the circuit tx_test.nr suite.
        "noir-tx-vectors" => {
            let path = "circuits/lib/src/tx_vectors.nr";
            std::fs::write(path, harness::noir_vectors::emit()).expect("write tx_vectors.nr");
            println!("wrote {path}");
        }
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

    // DA fold step (DOMAIN_DA=7, 3-input): acc' = P2([7, acc, msg]).
    println!(
        "da_fold(0, 42)         = {}",
        to_hex(&hasher.hash(&[fr_from_u64(7), fr_from_u64(0), fr_from_u64(42)]))
    );
}

fn sig_vectors() {
    use ark_ec::AffineRepr;
    use harness::keys::{coord_to_fr, sign_with_nonce, verify, Keypair};

    let hasher = Hasher::new();
    let gen = ark_grumpkin::Affine::generator();
    println!("gen_x   = {}", to_hex(&coord_to_fr(&gen.x)));
    println!("gen_y   = {}", to_hex(&coord_to_fr(&gen.y)));

    // Deterministic vector: sk = 7, k = 13, msg = 42 (raw; padding constants).
    let keypair = Keypair::from_sk_raw(ark_grumpkin::Fr::from(7u64));
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
    use harness::batch::{build_batch, make_signed_tx, DepositRequest};
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
    let txs = [
        make_signed_tx(&hasher, &alice, bob.pk_x(), 200, 0, false, &mut rng),
        make_signed_tx(&hasher, &bob, wd_field, 100, 0, true, &mut rng),
    ];
    let witness = build_batch(
        &hasher,
        &mut tree,
        2,
        4,
        &[
            DepositRequest { pk_x: alice.pk_x(), amount: 1000 },
            DepositRequest { pk_x: bob.pk_x(), amount: 500 },
        ],
        &txs,
    )
    .expect("demo batch must build");

    println!("old_root      = {}", to_hex(&witness.old_root));
    println!("new_root      = {}", to_hex(&witness.new_root));
    println!("deposit_hash  = {}", to_hex(&witness.deposit_hash));
    println!("withdraw_hash = {}", to_hex(&witness.withdraw_hash));
    println!("da_commitment = {}", to_hex(&witness.da_commitment));

    // Metadata for the contract test to replay the same scenario.
    let meta = serde_json::json!({
        "old_root": to_hex(&witness.old_root),
        "new_root": to_hex(&witness.new_root),
        "deposit_hash": to_hex(&witness.deposit_hash),
        "withdraw_hash": to_hex(&witness.withdraw_hash),
        "da_commitment": to_hex(&witness.da_commitment),
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

    // CLI-ready envelope (stellar contract invoke JSON arg conventions:
    // BytesN/Bytes as hex, Address as strkey, struct fields snake_case).
    let proof = std::fs::read(fixture_dir.join("proof")).unwrap();
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let envelope = serde_json::json!({
        "new_root": to_hex(&witness.new_root).trim_start_matches("0x"),
        "deposit_count": 2,
        "withdrawals": [ { "dest": wd_addr, "amount": "100" } ],
        "da_commitment": to_hex(&witness.da_commitment).trim_start_matches("0x"),
        "proof": hex(&proof),
    });
    std::fs::write(
        fixture_dir.join("envelope.json"),
        serde_json::to_string(&envelope).unwrap(),
    )
    .unwrap();
    println!("wrote fixtures/batch_n4/{{meta.json, envelope.json}}");
}

/// D deposits + N txs (N-2 transfers, 2 withdrawals) purely for cost-scaling
/// measurements; no meta/envelope needed (measured via the verify entrypoint).
fn demo_batch_sized(d: usize, n: usize, pkg: &str) {
    use harness::batch::{build_batch, make_signed_tx, DepositRequest};
    use harness::l1::address_to_field;
    use harness::prover;
    use harness::tree::Tree;
    use rand::SeedableRng;
    use std::collections::HashMap;

    let hasher = Hasher::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(2);

    let users: Vec<harness::keys::Keypair> = (0..d)
        .map(|i| harness::keys::Keypair::from_sk(ark_grumpkin::Fr::from(301u64 + i as u64)))
        .collect();
    let wd_field = address_to_field(&hasher, &wd_addr());

    let mut tree = Tree::new();
    let deposits: Vec<DepositRequest> = users
        .iter()
        .map(|u| DepositRequest { pk_x: u.pk_x(), amount: 1_000_000 })
        .collect();

    let mut nonces: HashMap<usize, u64> = HashMap::new();
    let mut next_nonce = |user: usize| {
        let n = nonces.entry(user).or_insert(0);
        let cur = *n;
        *n += 1;
        cur
    };

    let mut txs = Vec::new();
    for i in 0..n - 2 {
        let from = i % d;
        let to = &users[(i + 1) % d];
        let nonce = next_nonce(from);
        txs.push(make_signed_tx(&hasher, &users[from], to.pk_x(), 50 + i as u64, nonce, false, &mut rng));
    }
    let n0 = next_nonce(0);
    txs.push(make_signed_tx(&hasher, &users[0], wd_field, 77, n0, true, &mut rng));
    let n1 = next_nonce(1);
    txs.push(make_signed_tx(&hasher, &users[1], wd_field, 88, n1, true, &mut rng));

    let witness = build_batch(&hasher, &mut tree, d, n, &deposits, &txs).expect("demo batch must build");
    println!("new_root = {}", to_hex(&witness.new_root));

    let toml = prover::to_prover_toml(&witness);
    prover::prove(pkg, &toml).expect("prove pipeline failed");
}
