use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub rpc_url: String,
    pub network_passphrase: String,
    pub contract_id: String,
    pub token_id: String,
    /// S... secret key of the sequencer's Stellar account (pays batch fees).
    pub sequencer_secret: String,
    /// G... public address (optional; derived from an identity if absent).
    pub sequencer_address: Option<String>,
    pub db_path: PathBuf,
    pub listen_addr: String,
    /// Max seconds the oldest pending tx/deposit waits before a batch fires.
    pub batch_max_wait_secs: u64,
    /// Watcher/batcher poll interval.
    pub tick_secs: u64,
    /// Circuit package to prove (fixed shape D=4/N=16 for batch_n16).
    pub circuit_pkg: String,
    pub deposit_slots: usize,
    pub tx_slots: usize,
}

fn var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required env var {name}"))
}

fn var_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> Result<Config, String> {
        let circuit_pkg = var_or("CIRCUIT_PKG", "batch_n16");
        let (deposit_slots, tx_slots) = match circuit_pkg.as_str() {
            "batch_n4" => (2, 4),
            "batch_n16" => (4, 16),
            "batch_n64" => (8, 64),
            other => return Err(format!("unknown CIRCUIT_PKG {other}")),
        };
        Ok(Config {
            rpc_url: var_or("RPC_URL", "https://soroban-testnet.stellar.org"),
            network_passphrase: var_or("NETWORK_PASSPHRASE", "Test SDF Network ; September 2015"),
            contract_id: var("CONTRACT_ID")?,
            token_id: var("TOKEN_ID")?,
            sequencer_secret: var("SEQUENCER_SECRET")?,
            sequencer_address: std::env::var("SEQUENCER_ADDRESS").ok(),
            db_path: PathBuf::from(var_or("DB_PATH", "sequencer.db")),
            listen_addr: var_or("LISTEN_ADDR", "0.0.0.0:8080"),
            batch_max_wait_secs: var_or("BATCH_MAX_WAIT_SECS", "30").parse().map_err(|_| "bad BATCH_MAX_WAIT_SECS")?,
            tick_secs: var_or("TICK_SECS", "5").parse().map_err(|_| "bad TICK_SECS")?,
            circuit_pkg,
            deposit_slots,
            tx_slots,
        })
    }
}
