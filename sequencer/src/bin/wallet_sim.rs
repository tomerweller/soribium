//! Test client for the sequencer pipeline: makes L1 deposits via the stellar
//! CLI and signs/POSTs L2 transactions with the harness's Grumpkin keys.
//! Deterministic keys by small scalar so pk_x values match the repo fixtures.
//!
//! Usage:
//!   wallet-sim pk <sk>
//!   wallet-sim deposit <l2_pk_x_hex> <amount>          (funder = SEQ key)
//!   wallet-sim send <from_sk> <to_pk_x_hex> <amount> <nonce>
//!   wallet-sim withdraw <from_sk> <dest_strkey> <amount> <nonce>
//!
//! Env: SORIBIUM_URL (default http://127.0.0.1:8080), CONTRACT_ID, SEQ_KEY
//! (stellar CLI identity name or secret), plus standard stellar network vars.

use harness::batch::tx_message;
use harness::keys::{sign, Keypair};
use harness::l1::address_to_field;
use harness::poseidon::{to_hex, Fr, Hasher};

fn seq_url() -> String {
    std::env::var("SORIBIUM_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into())
}

fn keypair(sk: u64) -> Keypair {
    Keypair::from_sk(ark_grumpkin::Fr::from(sk))
}

fn post_tx(body: &serde_json::Value) {
    let url = format!("{}/tx", seq_url());
    let out = std::process::Command::new("curl")
        .args(["-sS", "-X", "POST", &url, "-H", "Content-Type: application/json", "-d", &body.to_string()])
        .output()
        .expect("curl");
    println!("{}", String::from_utf8_lossy(&out.stdout));
}

fn sig_json(sig: &harness::keys::Signature) -> serde_json::Value {
    let (s_lo, s_hi) = sig.s_limbs();
    serde_json::json!({
        "r_x": to_hex(&sig.r_x),
        "r_y": to_hex(&sig.r_y),
        "s_lo": to_hex(&s_lo),
        "s_hi": to_hex(&s_hi),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("");
    let hasher = Hasher::new();

    match cmd {
        "pk" => {
            let sk: u64 = args[2].parse().unwrap();
            let kp = keypair(sk);
            println!("pk_x={}", to_hex(&kp.pk_x()));
            println!("pk_y={}", to_hex(&kp.pk_y()));
        }
        "deposit" => {
            let l2_pk_x = &args[2];
            let amount = &args[3];
            let contract = std::env::var("CONTRACT_ID").expect("CONTRACT_ID");
            let key = std::env::var("SEQ_KEY").expect("SEQ_KEY");
            let rpc = std::env::var("RPC_URL").unwrap_or_else(|_| "https://soroban-testnet.stellar.org".into());
            let pass = std::env::var("NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".into());
            // `from` is the sequencer's own Stellar account (the funder).
            let addr = pubkey_of(&key);
            let l2_pk_x_bare = l2_pk_x.trim_start_matches("0x");
            let status = std::process::Command::new("stellar")
                .args([
                    "contract", "invoke", "--id", &contract, "--rpc-url", &rpc,
                    "--network-passphrase", &pass, "--source-account", &key, "--",
                    "deposit", "--from", &addr, "--l2_pk_x", l2_pk_x_bare, "--amount", amount,
                ])
                .status()
                .expect("stellar invoke");
            std::process::exit(status.code().unwrap_or(1));
        }
        "send" => {
            let from = keypair(args[2].parse().unwrap());
            let to: Fr = parse_hex(&args[3]);
            let amount: u64 = args[4].parse().unwrap();
            let nonce: u64 = args[5].parse().unwrap();
            let msg = tx_message(&hasher, from.pk_x(), to, amount, nonce, false);
            let sig = sign(&hasher, &from, msg, &mut rand::thread_rng());
            post_tx(&serde_json::json!({
                "from_pk_x": to_hex(&from.pk_x()),
                "from_pk_y": to_hex(&from.pk_y()),
                "to": to_hex(&to),
                "amount": amount.to_string(),
                "nonce": nonce,
                "is_withdraw": false,
                "sig": sig_json(&sig),
            }));
        }
        "withdraw" => {
            let from = keypair(args[2].parse().unwrap());
            let dest = &args[3];
            let amount: u64 = args[4].parse().unwrap();
            let nonce: u64 = args[5].parse().unwrap();
            let to_field = address_to_field(&hasher, dest);
            let msg = tx_message(&hasher, from.pk_x(), to_field, amount, nonce, true);
            let sig = sign(&hasher, &from, msg, &mut rand::thread_rng());
            post_tx(&serde_json::json!({
                "from_pk_x": to_hex(&from.pk_x()),
                "from_pk_y": to_hex(&from.pk_y()),
                "to": dest,
                "amount": amount.to_string(),
                "nonce": nonce,
                "is_withdraw": true,
                "sig": sig_json(&sig),
            }));
        }
        _ => {
            eprintln!("usage: wallet-sim pk|deposit|send|withdraw ...");
            std::process::exit(2);
        }
    }
}

fn parse_hex(s: &str) -> Fr {
    let body = s.trim_start_matches("0x");
    let bytes = hex::decode(body).expect("hex");
    let mut out = [0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

fn pubkey_of(key: &str) -> String {
    // `key` is a stellar CLI identity name; resolve to its G-address.
    let out = std::process::Command::new("stellar")
        .args(["keys", "address", key])
        .output()
        .expect("stellar keys address");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
