//! Chain access via the stellar CLI (proven pattern from scripts/e2e_local.sh)
//! plus raw JSON-RPC for transaction polling. The trait isolates a future
//! swap to a native RPC client.

use crate::hexutil::{parse_fr, HexError};
use harness::poseidon::Fr;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("cli: {0}")]
    Cli(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl From<HexError> for ChainError {
    fn from(e: HexError) -> Self {
        ChainError::Parse(format!("{e:?}"))
    }
}

pub trait StellarClient: Send + Sync {
    fn root(&self) -> Result<Fr, ChainError>;
    fn batch_num(&self) -> Result<u64, ChainError>;
    fn dep_tail(&self) -> Result<u64, ChainError>;
    fn get_pending_deposit(&self, seq: u64) -> Result<(Fr, u64), ChainError>;
    /// Sign + send submit_batch with the envelope JSON; returns when the CLI
    /// exits (success implies the tx was applied on-chain).
    fn submit_batch(&self, envelope_json: &str) -> Result<(), ChainError>;
}

/// Identity name the secret is registered under at boot. The raw S… secret
/// must never appear on a CLI argv (visible in `ps`/audit logs — issue #2
/// H4); it reaches the CLI exactly once, via environment, at registration.
const IDENTITY: &str = "soribium-seq-runtime";

pub struct CliClient {
    pub rpc_url: String,
    pub network_passphrase: String,
    pub contract_id: String,
    pub sequencer_address: String,
}

impl CliClient {
    pub fn new(cfg: &crate::config::Config) -> Result<Self, ChainError> {
        // Register the runtime identity from the secret — idempotently
        // (--overwrite tolerates a prior boot). The secret travels via env,
        // never argv; every later invoke uses the identity NAME.
        let o = Command::new("stellar")
            .args(["keys", "add", IDENTITY, "--secret-key", "--overwrite"])
            .env("SOROBAN_SECRET_KEY", &cfg.sequencer_secret)
            .env("STELLAR_SECRET_KEY", &cfg.sequencer_secret)
            .output()?;
        if !o.status.success() {
            return Err(ChainError::Cli(format!(
                "cannot register sequencer identity: {}",
                String::from_utf8_lossy(&o.stderr)
            )));
        }
        // Prefer the explicitly-provided public address (bootstrap knows it);
        // otherwise read it back from the registered identity.
        let sequencer_address = match &cfg.sequencer_address {
            Some(addr) if !addr.is_empty() => addr.clone(),
            _ => {
                let o = Command::new("stellar").args(["keys", "address", IDENTITY]).output()?;
                if !o.status.success() {
                    return Err(ChainError::Cli(format!(
                        "cannot resolve sequencer address: {}",
                        String::from_utf8_lossy(&o.stderr)
                    )));
                }
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
        };
        Ok(CliClient {
            rpc_url: cfg.rpc_url.clone(),
            network_passphrase: cfg.network_passphrase.clone(),
            contract_id: cfg.contract_id.clone(),
            sequencer_address,
        })
    }

    fn invoke(&self, send: bool, func_and_args: &[&str]) -> Result<String, ChainError> {
        let mut cmd = Command::new("stellar");
        cmd.args([
            "contract",
            "invoke",
            "--id",
            &self.contract_id,
            "--rpc-url",
            &self.rpc_url,
            "--network-passphrase",
            &self.network_passphrase,
            "--source-account",
            IDENTITY,
        ]);
        if !send {
            cmd.arg("--send=no");
        }
        cmd.arg("--");
        cmd.args(func_and_args);
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(ChainError::Cli(format!(
                "invoke {:?} failed: {}",
                func_and_args.first(),
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn read_u64(&self, func: &str) -> Result<u64, ChainError> {
        let out = self.invoke(false, &[func])?;
        out.trim_matches('"')
            .parse()
            .map_err(|_| ChainError::Parse(format!("{func} -> {out:?}")))
    }
}

impl StellarClient for CliClient {
    fn root(&self) -> Result<Fr, ChainError> {
        let out = self.invoke(false, &["root"])?;
        let hexstr = out.trim_matches('"');
        Ok(parse_fr(&format!("0x{hexstr}"))?)
    }

    fn batch_num(&self) -> Result<u64, ChainError> {
        self.read_u64("batch_num")
    }

    fn dep_tail(&self) -> Result<u64, ChainError> {
        self.read_u64("dep_tail")
    }

    fn get_pending_deposit(&self, seq: u64) -> Result<(Fr, u64), ChainError> {
        let out = self.invoke(false, &["get_pending_deposit", "--seq", &seq.to_string()])?;
        let v: serde_json::Value =
            serde_json::from_str(&out).map_err(|e| ChainError::Parse(e.to_string()))?;
        let pk_hex = v["pk_x"]
            .as_str()
            .ok_or_else(|| ChainError::Parse(format!("deposit {seq}: {out}")))?;
        let pk_x = parse_fr(&format!("0x{pk_hex}"))?;
        let amount_str = match &v["amount"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            other => return Err(ChainError::Parse(format!("deposit amount: {other}"))),
        };
        let amount: u64 = amount_str
            .parse()
            .map_err(|_| ChainError::Parse(format!("deposit amount: {amount_str}")))?;
        Ok((pk_x, amount))
    }

    fn submit_batch(&self, envelope_json: &str) -> Result<(), ChainError> {
        self.invoke(
            true,
            &[
                "submit_batch",
                "--sequencer",
                &self.sequencer_address,
                "--envelope",
                envelope_json,
            ],
        )?;
        Ok(())
    }
}
