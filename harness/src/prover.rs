//! Prover.toml emission + bb invocation via circuits/scripts/prove.sh.

use crate::batch::BatchWitness;
use crate::poseidon::{fr_from_u64, to_hex, Fr};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn fr_lit(fr: &Fr) -> String {
    format!("\"{}\"", to_hex(fr))
}

fn u64_lit(v: u64) -> String {
    fr_lit(&fr_from_u64(v))
}

fn bool_lit(v: bool) -> String {
    fr_lit(&fr_from_u64(v as u64))
}

fn siblings_lit(siblings: &[Fr]) -> String {
    let inner: Vec<String> = siblings.iter().map(fr_lit).collect();
    format!("[{}]", inner.join(", "))
}

/// Render the batch witness as a Prover.toml matching batch_nN/src/main.nr.
pub fn to_prover_toml(w: &BatchWitness) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "old_root = {}", fr_lit(&w.old_root));
    let _ = writeln!(out, "new_root = {}", fr_lit(&w.new_root));
    let _ = writeln!(out, "deposit_hash = {}", fr_lit(&w.deposit_hash));
    let _ = writeln!(out, "withdraw_hash = {}", fr_lit(&w.withdraw_hash));
    for d in &w.deposits {
        let _ = writeln!(out, "\n[[deposits]]");
        let _ = writeln!(out, "pk_x = {}", fr_lit(&d.pk_x));
        let _ = writeln!(out, "amount = {}", u64_lit(d.amount));
        let _ = writeln!(out, "index = {}", u64_lit(d.index as u64));
        let _ = writeln!(out, "old_pk_x = {}", fr_lit(&d.old_pk_x));
        let _ = writeln!(out, "old_balance = {}", u64_lit(d.old_balance));
        let _ = writeln!(out, "old_nonce = {}", u64_lit(d.old_nonce));
        let _ = writeln!(out, "siblings = {}", siblings_lit(&d.siblings));
        let _ = writeln!(out, "is_active = {}", bool_lit(d.is_active));
    }
    for t in &w.txs {
        let _ = writeln!(out, "\n[[txs]]");
        let _ = writeln!(out, "from_pk_x = {}", fr_lit(&t.from_pk_x));
        let _ = writeln!(out, "from_pk_y = {}", fr_lit(&t.from_pk_y));
        let _ = writeln!(out, "from_index = {}", u64_lit(t.from_index as u64));
        let _ = writeln!(out, "from_balance = {}", u64_lit(t.from_balance));
        let _ = writeln!(out, "from_nonce = {}", u64_lit(t.from_nonce));
        let _ = writeln!(out, "from_siblings = {}", siblings_lit(&t.from_siblings));
        let _ = writeln!(out, "to_field = {}", fr_lit(&t.to_field));
        let _ = writeln!(out, "to_index = {}", u64_lit(t.to_index as u64));
        let _ = writeln!(out, "to_balance = {}", fr_lit(&t.to_balance_or_leaf));
        let _ = writeln!(out, "to_nonce = {}", u64_lit(t.to_nonce));
        let _ = writeln!(out, "to_siblings = {}", siblings_lit(&t.to_siblings));
        let _ = writeln!(out, "amount = {}", u64_lit(t.amount));
        let _ = writeln!(out, "is_withdraw = {}", bool_lit(t.is_withdraw));
        let _ = writeln!(out, "is_active = {}", bool_lit(t.is_active));
        let (s_lo, s_hi) = t.sig.s_limbs();
        let _ = writeln!(out, "[txs.sig]");
        let _ = writeln!(out, "r_x = {}", fr_lit(&t.sig.r_x));
        let _ = writeln!(out, "r_y = {}", fr_lit(&t.sig.r_y));
        let _ = writeln!(out, "s_lo = {}", fr_lit(&s_lo));
        let _ = writeln!(out, "s_hi = {}", fr_lit(&s_hi));
    }
    out
}

pub fn repo_root() -> PathBuf {
    // harness/ -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Write Prover.toml for `pkg` and run the full prove pipeline (nargo + bb +
/// fixture staging). Requires nargo/bb on PATH or at ~/.nargo/bin, ~/.bb.
pub fn prove(pkg: &str, prover_toml: &str) -> std::io::Result<()> {
    let root = repo_root();
    let toml_path = root.join("circuits").join(pkg).join("Prover.toml");
    std::fs::write(&toml_path, prover_toml)?;

    let script = root.join("circuits/scripts/prove.sh");
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!(
        "{home}/.nargo/bin:{home}/.bb:{}",
        std::env::var("PATH").unwrap_or_default()
    );
    let status = std::process::Command::new(&script)
        .arg(pkg)
        .env("PATH", path)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("prove.sh {pkg} failed: {status}")));
    }
    Ok(())
}
