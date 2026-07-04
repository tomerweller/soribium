//! Deposit watcher: polls the contract's FIFO queue (dep_tail + a local
//! cursor) rather than getEvents, which has an RPC retention window a downed
//! sequencer could miss. Queue seqs are exactly-once by construction; the
//! engine dedupes via INSERT OR IGNORE, so a restart with a stale cursor is
//! harmless.

use crate::engine::Command;
use crate::stellar::StellarClient;
use harness::poseidon::Fr;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tokio::sync::oneshot;

pub async fn run(
    engine: mpsc::Sender<Command>,
    client: Arc<dyn StellarClient>,
    tick_secs: u64,
    mut cursor: u64,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(tick_secs));
    loop {
        interval.tick().await;

        let c = client.clone();
        let tail = match tokio::task::spawn_blocking(move || c.dep_tail()).await {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => {
                tracing::warn!(%e, "dep_tail poll failed");
                continue;
            }
            Err(e) => {
                tracing::error!(%e, "dep_tail task panicked");
                continue;
            }
        };
        if tail <= cursor {
            continue;
        }

        let mut observed: Vec<(u64, Fr, u64)> = Vec::new();
        let mut failed = false;
        for seq in cursor..tail {
            let c = client.clone();
            match tokio::task::spawn_blocking(move || c.get_pending_deposit(seq)).await {
                Ok(Ok((pk_x, amount))) => observed.push((seq, pk_x, amount)),
                Ok(Err(e)) => {
                    tracing::warn!(seq, %e, "get_pending_deposit failed; will retry next tick");
                    failed = true;
                    break;
                }
                Err(e) => {
                    tracing::error!(seq, %e, "get_pending_deposit task panicked");
                    failed = true;
                    break;
                }
            }
        }

        if observed.is_empty() {
            continue;
        }
        let advance_to = cursor + observed.len() as u64;
        if report(&engine, observed).await {
            cursor = advance_to;
        }
        if failed {
            // Leave the rest for the next tick.
            continue;
        }
    }
}

/// Report observed deposits to the engine, awaiting its ack.
async fn report(engine: &mpsc::Sender<Command>, deposits: Vec<(u64, Fr, u64)>) -> bool {
    let (tx, rx) = oneshot::channel();
    if engine.send(Command::ObservedDeposits(deposits, tx)).is_err() {
        return false;
    }
    matches!(rx.await, Ok(Ok(())))
}
