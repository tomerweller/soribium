mod api;
mod batcher;
mod config;
mod db;
mod engine;
mod hexutil;
mod stellar;
mod watcher;

use config::Config;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sequencer=info,tower_http=warn".into()),
        )
        .init();

    if let Err(e) = run().await {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    // One-shot subcommand (needs no config/env): print the empty-tree genesis
    // root and exit. Used by the bootstrap script.
    if std::env::args().nth(1).as_deref() == Some("genesis-root") {
        let hasher = harness::poseidon::Hasher::new();
        let tree = harness::tree::Tree::new();
        println!("{}", hexutil::fr_hex(&tree.root(&hasher)));
        return Ok(());
    }

    let cfg = Config::from_env()?;
    tracing::info!(contract = %cfg.contract_id, circuit = %cfg.circuit_pkg, "starting Soribium sequencer");

    let conn = db::open(&cfg.db_path).map_err(|e| format!("db open: {e}"))?;

    // Chain client + boot reconciliation (contract is the source of truth).
    let client: Arc<dyn stellar::StellarClient> =
        Arc::new(stellar::CliClient::new(&cfg).map_err(|e| format!("stellar client: {e}"))?);
    let chain_root = client.root().map_err(|e| format!("read root: {e}"))?;
    let chain_batch_num = client.batch_num().map_err(|e| format!("read batch_num: {e}"))?;
    let dep_cursor = {
        // Cursor persists in meta; default to 0 on a fresh DB.
        db::meta_get_u64(&conn, "dep_cursor").map_err(|e| e.to_string())?
    };

    let boot = engine::load_and_reconcile(&conn, &chain_root, chain_batch_num)?;
    tracing::info!(
        chain_batch_num,
        chain_synced = boot.chain_synced,
        "reconciled local state against chain"
    );

    let engine = engine::spawn(cfg.clone(), conn, boot.tree, boot.chain_synced);

    // Background tasks: deposit watcher + batch pipeline.
    tokio::spawn(watcher::run(engine.clone(), client.clone(), cfg.tick_secs, dep_cursor));
    tokio::spawn(batcher::run(engine.clone(), client.clone(), cfg.clone()));

    // HTTP server.
    let state = api::AppState { engine, cfg: cfg.clone() };
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr)
        .await
        .map_err(|e| format!("bind {}: {e}", cfg.listen_addr))?;
    tracing::info!(addr = %cfg.listen_addr, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("serve: {e}"))?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
