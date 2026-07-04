//! Batch pipeline driver. Ticks on an interval; when the engine says a batch
//! is ready it runs the blocking prove (bb) off the async runtime, then
//! submits and confirms. The engine owns all state; this loop only
//! orchestrates the irreversible steps and hands results back.
//!
//! State machine (per batch row, owned by the engine):
//!   building -> proving -> proved -> submitting -> submitted -> confirmed
//!                      \-> failed (inputs requeued)

use crate::config::Config;
use crate::engine::{ApiError, BatchJob, Command};
use crate::stellar::StellarClient;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tokio::sync::oneshot;

pub async fn run(
    engine: mpsc::Sender<Command>,
    client: Arc<dyn StellarClient>,
    cfg: Config,
) {
    // Resume any batch left mid-pipeline by a crash before the normal loop.
    if let Some((batch_num, status)) = inflight(&engine).await {
        tracing::info!(batch_num, %status, "resuming inflight batch on boot");
        resume(&engine, &client, &cfg, batch_num, &status).await;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(cfg.tick_secs));
    loop {
        interval.tick().await;
        match try_build(&engine).await {
            Ok(Some(job)) => {
                run_pipeline(&engine, &client, &cfg, job).await;
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(%e, "try_build_batch failed"),
        }
    }
}

async fn run_pipeline(
    engine: &mpsc::Sender<Command>,
    client: &Arc<dyn StellarClient>,
    cfg: &Config,
    job: BatchJob,
) {
    let batch_num = job.batch_num;

    // --- prove (blocking; off the async runtime) ---
    let pkg = cfg.circuit_pkg.clone();
    let toml = job.prover_toml.clone();
    let proof_result = tokio::task::spawn_blocking(move || {
        std::env::set_var("STAGE_FIXTURES", "0");
        let out_dir = harness::prover::prove(&pkg, &toml)?;
        let proof = std::fs::read(out_dir.join("proof"))?;
        let public_inputs = std::fs::read(out_dir.join("public_inputs"))?;
        Ok::<_, std::io::Error>((proof, public_inputs))
    })
    .await;

    let (proof, public_inputs) = match proof_result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            fail(engine, batch_num, &format!("prove failed: {e}")).await;
            return;
        }
        Err(e) => {
            fail(engine, batch_num, &format!("prove task panicked: {e}")).await;
            return;
        }
    };

    // --- validate + record proof; check bb PIs equal the built ones ---
    let envelope_json = match record_proof(engine, batch_num, proof, public_inputs).await {
        Ok(env) => env,
        Err(e) => {
            fail(engine, batch_num, &format!("record proof failed: {e}")).await;
            return;
        }
    };

    submit_and_confirm(engine, client, cfg, batch_num, envelope_json).await;
}

async fn submit_and_confirm(
    engine: &mpsc::Sender<Command>,
    client: &Arc<dyn StellarClient>,
    cfg: &Config,
    batch_num: u64,
    envelope_json: String,
) {
    // Mark submitting BEFORE the CLI call so a crash mid-send is detectable.
    if let Err(e) = ask(engine, |r| Command::MarkSubmitting(batch_num, r)).await {
        tracing::error!(batch_num, %e, "mark submitting failed");
        return;
    }

    let c = client.clone();
    let env = envelope_json.clone();
    let submit = tokio::task::spawn_blocking(move || c.submit_batch(&env)).await;
    match submit {
        Ok(Ok(())) => {
            let _ = ask(engine, |r| Command::MarkSubmitted(batch_num, None, r)).await;
        }
        Ok(Err(e)) => {
            // Submission may or may not have landed; confirm by polling root.
            tracing::warn!(batch_num, %e, "submit_batch returned error; verifying via root");
        }
        Err(e) => tracing::error!(batch_num, %e, "submit task panicked"),
    }

    confirm(engine, client, cfg, batch_num).await;
}

/// Poll the chain root until it equals this batch's new_root, then tell the
/// engine to apply the batch. Bounded retries; on timeout the batch stays
/// 'submitted' and boot recovery re-checks it.
async fn confirm(
    engine: &mpsc::Sender<Command>,
    client: &Arc<dyn StellarClient>,
    cfg: &Config,
    batch_num: u64,
) {
    for attempt in 0..30u32 {
        tokio::time::sleep(Duration::from_secs(cfg.tick_secs)).await;
        let c = client.clone();
        let chain_bn = match tokio::task::spawn_blocking(move || c.batch_num()).await {
            Ok(Ok(bn)) => bn,
            _ => continue,
        };
        if chain_bn >= batch_num {
            match ask(engine, |r| Command::ConfirmBatch(batch_num, r)).await {
                Ok(()) => {
                    tracing::info!(batch_num, "batch confirmed on chain");
                    return;
                }
                Err(e) => {
                    tracing::error!(batch_num, %e, "confirm apply failed");
                    return;
                }
            }
        }
        tracing::debug!(batch_num, attempt, chain_bn, "awaiting confirmation");
    }
    tracing::warn!(batch_num, "confirmation timed out; boot recovery will re-check");
}

async fn resume(
    engine: &mpsc::Sender<Command>,
    client: &Arc<dyn StellarClient>,
    cfg: &Config,
    batch_num: u64,
    status: &str,
) {
    match status {
        // Proof exists; re-submitting is safe (proof binds old_root, a
        // double-land fails verification) — go straight to submit+confirm.
        "proved" | "submitting" | "submitted" => {
            let envelope = match ask(engine, |r| Command::MarkSubmitting(batch_num, r)).await {
                Ok(env) => env,
                Err(e) => {
                    tracing::error!(batch_num, %e, "resume mark submitting failed");
                    return;
                }
            };
            submit_and_confirm(engine, client, cfg, batch_num, envelope).await;
        }
        other => {
            // 'proving' and anything else pre-proof: the prove artifacts are
            // gone; fail + requeue (boot reconcile may already have done this).
            tracing::warn!(batch_num, status = other, "resume: failing pre-proof batch");
            fail(engine, batch_num, "interrupted before proof").await;
        }
    }
}

// ---- engine command helpers ----

async fn inflight(engine: &mpsc::Sender<Command>) -> Option<(u64, String)> {
    let (tx, rx) = oneshot::channel();
    engine.send(Command::GetInflight(tx)).ok()?;
    rx.await.ok().flatten()
}

async fn try_build(engine: &mpsc::Sender<Command>) -> Result<Option<BatchJob>, ApiError> {
    ask_r(engine, Command::TryBuildBatch).await
}

async fn record_proof(
    engine: &mpsc::Sender<Command>,
    batch_num: u64,
    proof: Vec<u8>,
    public_inputs: Vec<u8>,
) -> Result<String, ApiError> {
    let (tx, rx) = oneshot::channel();
    engine
        .send(Command::RecordProof { batch_num, proof, public_inputs, reply: tx })
        .map_err(|_| ApiError::Internal("engine offline".into()))?;
    rx.await.map_err(|_| ApiError::Internal("engine dropped reply".into()))?
}

async fn fail(engine: &mpsc::Sender<Command>, batch_num: u64, reason: &str) {
    let _ = ask(engine, |r| Command::FailBatch(batch_num, reason.to_string(), r)).await;
}

async fn ask<T, F>(engine: &mpsc::Sender<Command>, build: F) -> Result<T, ApiError>
where
    F: FnOnce(oneshot::Sender<Result<T, ApiError>>) -> Command,
{
    let (tx, rx) = oneshot::channel();
    engine
        .send(build(tx))
        .map_err(|_| ApiError::Internal("engine offline".into()))?;
    rx.await.map_err(|_| ApiError::Internal("engine dropped reply".into()))?
}

async fn ask_r<T, F>(engine: &mpsc::Sender<Command>, build: F) -> Result<T, ApiError>
where
    F: FnOnce(oneshot::Sender<Result<T, ApiError>>) -> Command,
{
    ask(engine, build).await
}
