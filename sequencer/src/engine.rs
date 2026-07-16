//! The engine actor: a single OS thread owning the account tree, the SQLite
//! connection, and every state mutation. HTTP handlers and the watcher/
//! batcher threads talk to it over an mpsc channel — this serializes
//! admission checks against batch building (no TOCTOU) and sidesteps the
//! fact that neither `soroban_sdk::Env` (inside Hasher) nor
//! `rusqlite::Connection` can be shared across threads.
//!
//! A fresh `Hasher` (soroban Env) is created per command: the Env's host-
//! object table grows monotonically, so a long-lived one would leak.

use crate::config::Config;
use crate::db;
use crate::hexutil::{fr_hex, parse_fr};
use harness::batch::{build_batch, tx_message, BuildError, DepositRequest, SignedTx};
use harness::keys::{pk_from_coords, verify, Signature};
use harness::l1::address_to_field;
use harness::poseidon::{Fr, Hasher, FR_ZERO};
use harness::tree::{Account, Tree};
use rusqlite::Connection;
use std::sync::mpsc;
use tokio::sync::oneshot;

// ---------- wire/result types ----------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WireSig {
    pub r_x: String,
    pub r_y: String,
    pub s_lo: String,
    pub s_hi: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WireTx {
    pub from_pk_x: String,
    pub from_pk_y: String,
    /// Transfer: recipient pk_x hex. Withdrawal: destination strkey.
    pub to: String,
    pub amount: String,
    pub nonce: u64,
    pub is_withdraw: bool,
    pub sig: WireSig,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TxReceipt {
    pub id: i64,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub pk_x: String,
    pub index: u32,
    pub balance: String,
    pub nonce: u64,
    pub pending_nonce: u64,
    pub pending_out: String,
    pub root: String,
    pub batch_num: u64,
    pub siblings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusInfo {
    pub root: String,
    pub batch_num: u64,
    pub pending_txs: u64,
    pub pending_deposits: u64,
    pub contract_id: String,
    pub inflight_batch: Option<InflightInfo>,
    pub chain_synced: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InflightInfo {
    pub batch_num: u64,
    pub status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("BAD_FIELD: {0}")]
    BadField(String),
    #[error("BAD_SIGNATURE")]
    BadSignature,
    #[error("NONCE_MISMATCH: expected {expected}")]
    NonceMismatch { expected: u64 },
    #[error("INSUFFICIENT_BALANCE: available {available}")]
    InsufficientBalance { available: u64 },
    #[error("RECIPIENT_UNKNOWN")]
    RecipientUnknown,
    #[error("ACCOUNT_UNKNOWN")]
    AccountUnknown,
    #[error("NOT_FOUND")]
    NotFound,
    #[error("internal: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

/// Work order handed to the batcher thread once a batch row is persisted.
/// The bb public_inputs are validated against the persisted batch row in
/// `record_proof`, so nothing beyond the toml needs to travel here.
#[derive(Debug)]
pub struct BatchJob {
    pub batch_num: u64,
    pub prover_toml: String,
}

pub enum Command {
    SubmitTx(WireTx, oneshot::Sender<Result<TxReceipt, ApiError>>),
    GetAccount(String, oneshot::Sender<Result<AccountInfo, ApiError>>),
    GetStatus(oneshot::Sender<StatusInfo>),
    GetHistory(String, oneshot::Sender<Result<Vec<db::HistoryEntry>, ApiError>>),
    GetDa(u64, oneshot::Sender<Result<serde_json::Value, ApiError>>),
    GetBatches(oneshot::Sender<Vec<serde_json::Value>>),
    /// From the watcher: newly observed L1 deposits (seq, pk_x, amount).
    ObservedDeposits(Vec<(u64, Fr, u64)>, oneshot::Sender<Result<(), ApiError>>),
    /// From the batcher tick: build a batch if trigger conditions hold.
    TryBuildBatch(oneshot::Sender<Result<Option<BatchJob>, ApiError>>),
    /// From the batcher: bb finished; validate + persist the proof.
    RecordProof {
        batch_num: u64,
        proof: Vec<u8>,
        public_inputs: Vec<u8>,
        reply: oneshot::Sender<Result<String, ApiError>>, // envelope_json for submission
    },
    MarkSubmitting(u64, oneshot::Sender<Result<String, ApiError>>), // -> envelope_json
    MarkSubmitted(u64, Option<String>, oneshot::Sender<Result<(), ApiError>>),
    /// From the batcher: chain root now equals the batch's new_root.
    ConfirmBatch(u64, oneshot::Sender<Result<(), ApiError>>),
    /// Batch failed pre-submission; requeue its inputs.
    FailBatch(u64, String, oneshot::Sender<Result<(), ApiError>>),
    /// Resume state for the batcher after boot.
    GetInflight(oneshot::Sender<Option<(u64, String)>>),
}

pub struct Engine {
    cfg: Config,
    conn: Connection,
    tree: Tree,
    chain_synced: bool,
}

pub fn spawn(cfg: Config, conn: Connection, tree: Tree, chain_synced: bool) -> mpsc::Sender<Command> {
    let (tx, rx) = mpsc::channel::<Command>();
    std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let mut engine = Engine { cfg, conn, tree, chain_synced };
            while let Ok(cmd) = rx.recv() {
                engine.handle(cmd);
            }
        })
        .expect("spawn engine thread");
    tx
}

impl Engine {
    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::SubmitTx(tx, reply) => {
                let _ = reply.send(self.submit_tx(tx));
            }
            Command::GetAccount(pk_hex, reply) => {
                let _ = reply.send(self.get_account(&pk_hex));
            }
            Command::GetStatus(reply) => {
                let _ = reply.send(self.get_status());
            }
            Command::GetHistory(pk_hex, reply) => {
                let _ = reply.send(self.get_history(&pk_hex));
            }
            Command::GetDa(batch_num, reply) => {
                let _ = reply.send(self.get_da(batch_num));
            }
            Command::GetBatches(reply) => {
                let _ = reply.send(self.get_batches());
            }
            Command::ObservedDeposits(deps, reply) => {
                let _ = reply.send(self.observed_deposits(deps));
            }
            Command::TryBuildBatch(reply) => {
                let _ = reply.send(self.try_build_batch());
            }
            Command::RecordProof { batch_num, proof, public_inputs, reply } => {
                let _ = reply.send(self.record_proof(batch_num, proof, public_inputs));
            }
            Command::MarkSubmitting(batch_num, reply) => {
                let _ = reply.send(self.mark_submitting(batch_num));
            }
            Command::MarkSubmitted(batch_num, tx_hash, reply) => {
                let _ = reply.send(
                    db::batch_set_submitted(&self.conn, batch_num, tx_hash.as_deref())
                        .map_err(Into::into),
                );
            }
            Command::ConfirmBatch(batch_num, reply) => {
                let _ = reply.send(self.confirm_batch(batch_num));
            }
            Command::FailBatch(batch_num, reason, reply) => {
                let _ = reply.send(self.fail_batch(batch_num, &reason));
            }
            Command::GetInflight(reply) => {
                let inflight = db::inflight_batch(&self.conn)
                    .ok()
                    .flatten()
                    .map(|b| (b.batch_num, b.status));
                let _ = reply.send(inflight);
            }
        }
    }

    // ---------- reads ----------

    fn confirmed_batch_num(&self) -> u64 {
        db::meta_get_u64(&self.conn, "confirmed_batch_num").unwrap_or(0)
    }

    fn get_account(&self, pk_hex: &str) -> Result<AccountInfo, ApiError> {
        let hasher = Hasher::new();
        let pk_x = parse_fr(pk_hex).map_err(|e| ApiError::BadField(format!("pk_x: {e:?}")))?;
        let index = self.tree.find(&pk_x).ok_or(ApiError::AccountUnknown)?;
        let account = self.tree.get(index).unwrap().clone();
        let (siblings, _) = self.tree.path(&hasher, index);
        let pending = db::mempool_pending_for(&self.conn, &pk_x)?;
        let pending_out: u64 = pending.iter().map(|t| t.amount).sum();
        Ok(AccountInfo {
            pk_x: fr_hex(&pk_x),
            index,
            balance: account.balance.to_string(),
            nonce: account.nonce,
            pending_nonce: account.nonce + pending.len() as u64,
            pending_out: pending_out.to_string(),
            root: fr_hex(&self.tree.root(&hasher)),
            batch_num: self.confirmed_batch_num(),
            siblings: siblings.iter().map(fr_hex).collect(),
        })
    }

    fn get_status(&self) -> StatusInfo {
        let hasher = Hasher::new();
        StatusInfo {
            root: fr_hex(&self.tree.root(&hasher)),
            batch_num: self.confirmed_batch_num(),
            pending_txs: db::mempool_count_pending(&self.conn).unwrap_or(0),
            pending_deposits: db::deposits_count_pending(&self.conn).unwrap_or(0),
            contract_id: self.cfg.contract_id.clone(),
            inflight_batch: db::inflight_batch(&self.conn)
                .ok()
                .flatten()
                .map(|b| InflightInfo { batch_num: b.batch_num, status: b.status }),
            chain_synced: self.chain_synced,
        }
    }

    fn get_history(&self, pk_hex: &str) -> Result<Vec<db::HistoryEntry>, ApiError> {
        let pk_x = parse_fr(pk_hex).map_err(|e| ApiError::BadField(format!("pk_x: {e:?}")))?;
        Ok(db::history_for(&self.conn, &pk_x, 100)?)
    }

    fn get_da(&self, batch_num: u64) -> Result<serde_json::Value, ApiError> {
        let batch = db::get_batch(&self.conn, batch_num)?.ok_or(ApiError::NotFound)?;
        if batch.status != "confirmed" {
            return Err(ApiError::NotFound);
        }
        let mut blob: serde_json::Value =
            serde_json::from_str(&batch.blob_json).map_err(|e| ApiError::Internal(e.to_string()))?;
        blob["proof"] = serde_json::Value::String(hex::encode(batch.proof.unwrap_or_default()));
        blob["tx_hash"] = match batch.tx_hash {
            Some(h) => serde_json::Value::String(h),
            None => serde_json::Value::Null,
        };
        Ok(blob)
    }

    fn get_batches(&self) -> Vec<serde_json::Value> {
        db::batch_list(&self.conn, 50)
            .unwrap_or_default()
            .into_iter()
            .map(|b| {
                serde_json::json!({
                    "batch_num": b.batch_num,
                    "old_root": fr_hex(&b.old_root),
                    "new_root": fr_hex(&b.new_root),
                    "deposit_count": b.deposit_count,
                    "da_commitment": fr_hex(&b.da_commitment),
                    "status": b.status,
                    "tx_hash": b.tx_hash,
                })
            })
            .collect()
    }

    // ---------- mempool admission ----------

    fn submit_tx(&mut self, tx: WireTx) -> Result<TxReceipt, ApiError> {
        let hasher = Hasher::new();
        let bad = |field: &str| ApiError::BadField(field.to_string());

        let from_pk_x = parse_fr(&tx.from_pk_x).map_err(|_| bad("from_pk_x"))?;
        let from_pk_y = parse_fr(&tx.from_pk_y).map_err(|_| bad("from_pk_y"))?;
        let amount: u64 = tx.amount.parse().map_err(|_| bad("amount"))?;
        if amount == 0 {
            return Err(bad("amount"));
        }

        // Idempotent resubmission: same (sender, nonce) returns the original.
        if let Some((id, status)) = db::mempool_find(&self.conn, &from_pk_x, tx.nonce)? {
            return Ok(TxReceipt { id, status });
        }

        let (to_field, withdraw_dest) = if tx.is_withdraw {
            if tx.to.len() != 56 || !(tx.to.starts_with('G') || tx.to.starts_with('C')) {
                return Err(bad("to: expected 56-char strkey"));
            }
            (address_to_field(&hasher, &tx.to), Some(tx.to.clone()))
        } else {
            let to = parse_fr(&tx.to).map_err(|_| bad("to"))?;
            if to == FR_ZERO {
                return Err(bad("to"));
            }
            (to, None)
        };

        // Signature (pk from untrusted coordinates; message per DESIGN.md).
        let pk = pk_from_coords(&from_pk_x, &from_pk_y).ok_or(ApiError::BadSignature)?;
        let sig_r_x = parse_fr(&tx.sig.r_x).map_err(|_| bad("sig.r_x"))?;
        let sig_r_y = parse_fr(&tx.sig.r_y).map_err(|_| bad("sig.r_y"))?;
        let sig_s_lo = parse_fr(&tx.sig.s_lo).map_err(|_| bad("sig.s_lo"))?;
        let sig_s_hi = parse_fr(&tx.sig.s_hi).map_err(|_| bad("sig.s_hi"))?;
        let sig = Signature::from_limbs(sig_r_x, sig_r_y, sig_s_lo, sig_s_hi)
            .ok_or(ApiError::BadSignature)?;
        let msg = tx_message(&hasher, from_pk_x, to_field, amount, tx.nonce, tx.is_withdraw);
        if !verify(&hasher, &pk, msg, &sig) {
            return Err(ApiError::BadSignature);
        }

        // Nonce and balance against confirmed state + mempool shadow.
        let sender_index = self.tree.find(&from_pk_x).ok_or(ApiError::AccountUnknown)?;
        let sender = self.tree.get(sender_index).unwrap().clone();
        let pending = db::mempool_pending_for(&self.conn, &from_pk_x)?;
        let expected_nonce = sender.nonce + pending.len() as u64;
        if tx.nonce != expected_nonce {
            return Err(ApiError::NonceMismatch { expected: expected_nonce });
        }
        let pending_out: u64 = pending.iter().map(|t| t.amount).sum();
        let available = sender.balance.saturating_sub(pending_out);
        if amount > available {
            return Err(ApiError::InsufficientBalance { available });
        }

        // Transfers: recipient must exist now or be created by a pending deposit.
        if !tx.is_withdraw
            && self.tree.find(&to_field).is_none()
            && !db::deposits_pending_pk(&self.conn, &to_field)?
        {
            return Err(ApiError::RecipientUnknown);
        }

        let id = db::insert_mempool(
            &self.conn,
            &from_pk_x,
            &from_pk_y,
            &to_field,
            withdraw_dest.as_deref(),
            amount,
            tx.nonce,
            tx.is_withdraw,
            [&sig_r_x, &sig_r_y, &sig_s_lo, &sig_s_hi],
        )?;
        Ok(TxReceipt { id, status: "pending".into() })
    }

    fn observed_deposits(&mut self, deps: Vec<(u64, Fr, u64)>) -> Result<(), ApiError> {
        let mut max_seq: Option<u64> = None;
        for (seq, pk_x, amount) in deps {
            max_seq = Some(max_seq.map_or(seq, |m| m.max(seq)));
            if db::insert_deposit(&self.conn, seq, &pk_x, amount)? {
                tracing::info!(seq, amount, "observed L1 deposit");
                // Jam alarms (known protocol limitation, see DESIGN.md).
                if let Some(idx) = self.tree.find(&pk_x) {
                    let bal = self.tree.get(idx).unwrap().balance;
                    if bal.checked_add(amount).is_none() {
                        tracing::error!(seq, "DEPOSIT JAM: balance would exceed u64; FIFO is stuck");
                    }
                } else if self.tree.free_index().is_none() {
                    tracing::error!(seq, "DEPOSIT JAM: tree full (256 accounts); FIFO is stuck");
                }
            }
        }
        // Persist the watcher cursor so a restart doesn't try to re-fetch
        // deposits that may already be dequeued (get_pending_deposit traps).
        if let Some(seq) = max_seq {
            db::meta_set(&self.conn, "dep_cursor", &(seq + 1).to_string())?;
        }
        Ok(())
    }

    // ---------- batch pipeline ----------

    fn try_build_batch(&mut self) -> Result<Option<BatchJob>, ApiError> {
        if !self.chain_synced {
            return Ok(None);
        }
        if db::inflight_batch(&self.conn)?.is_some() {
            return Ok(None);
        }

        let pending_txs = db::mempool_count_pending(&self.conn)?;
        let pending_deps = db::deposits_count_pending(&self.conn)?;
        if pending_txs == 0 && pending_deps == 0 {
            return Ok(None);
        }
        let oldest_age = [
            db::mempool_oldest_pending_age(&self.conn)?,
            db::deposits_oldest_pending_age(&self.conn)?,
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);

        // Batch eagerly: whenever more than one payment is pending, build on
        // this tick (no waiting to fill the batch or hit the timer). The
        // deposit-queue-full and max-wait conditions remain as fallbacks so a
        // lone single payment, or deposit-only activity with no payments,
        // still settles instead of stranding.
        let many_payments = pending_txs > 1;
        let deposits_full = pending_deps >= self.cfg.deposit_slots as u64;
        let waited = (oldest_age as u64) >= self.cfg.batch_max_wait_secs;
        if !(many_payments || deposits_full || waited) {
            return Ok(None);
        }

        self.build_batch_now()
    }

    fn build_batch_now(&mut self) -> Result<Option<BatchJob>, ApiError> {
        let hasher = Hasher::new();
        let deposits = db::deposits_pending(&self.conn, self.cfg.deposit_slots)?;
        let mut candidates = db::mempool_pending(&self.conn, self.cfg.tx_slots * 4)?;

        // Cap withdrawals per batch (contract MAX_WITHDRAWALS = 8).
        let mut txs: Vec<db::MempoolRow> = Vec::new();
        let mut withdrawals = 0usize;
        candidates.retain(|t| {
            if txs.len() >= self.cfg.tx_slots {
                return true;
            }
            if t.is_withdraw {
                if withdrawals >= 8 {
                    return true;
                }
                withdrawals += 1;
            }
            txs.push(t.clone());
            false
        });

        let dep_requests: Vec<DepositRequest> = deposits
            .iter()
            .map(|d| DepositRequest { pk_x: d.pk_x, amount: d.amount })
            .collect();

        // Build on a clone; the live tree only advances at confirmation.
        loop {
            let mut work_tree = self.clone_tree();
            let signed: Vec<SignedTx> = txs.iter().map(row_to_signed).collect();
            match build_batch(
                &hasher,
                &mut work_tree,
                self.cfg.deposit_slots,
                self.cfg.tx_slots,
                &dep_requests,
                &signed,
            ) {
                Ok(witness) => {
                    let batch_num = self.confirmed_batch_num() + 1;
                    let blob = blob_json(batch_num, &witness, &deposits, &txs);
                    let envelope = envelope_json(&witness, deposits.len() as u32, &txs);
                    let prover_toml = harness::prover::to_prover_toml(&witness);
                    db::insert_batch(
                        &self.conn,
                        batch_num,
                        &witness.old_root,
                        &witness.new_root,
                        deposits.len() as u32,
                        &witness.da_commitment,
                        &blob,
                        &envelope,
                    )?;
                    db::deposits_set_status(
                        &self.conn,
                        &deposits.iter().map(|d| d.seq).collect::<Vec<_>>(),
                        "batching",
                        Some(batch_num),
                    )?;
                    db::mempool_set_status(
                        &self.conn,
                        &txs.iter().map(|t| t.id).collect::<Vec<_>>(),
                        "batching",
                        Some(batch_num),
                        None,
                    )?;
                    tracing::info!(batch_num, txs = txs.len(), deposits = deposits.len(), "batch built");
                    return Ok(Some(BatchJob { batch_num, prover_toml }));
                }
                Err(BuildError::TreeFull) | Err(BuildError::BalanceOverflow { .. })
                    if !deposits.is_empty() =>
                {
                    // Deposit jam: cannot make progress at all (FIFO prefix is
                    // mandatory). Alarm and stop trying this tick.
                    tracing::error!("deposit jam while building; batching stalled");
                    return Ok(None);
                }
                Err(err) => {
                    // Evict the offending tx and retry without it.
                    let idx = match err {
                        BuildError::SenderNotFound { tx_index }
                        | BuildError::NonceMismatch { tx_index, .. }
                        | BuildError::InsufficientBalance { tx_index, .. }
                        | BuildError::RecipientNotFound { tx_index }
                        | BuildError::BadSignature { tx_index } => tx_index,
                        other => {
                            tracing::error!(?other, "unbuildable batch");
                            return Ok(None);
                        }
                    };
                    let evicted = txs.remove(idx);
                    tracing::warn!(id = evicted.id, ?err, "rejecting mempool tx");
                    db::mempool_set_status(
                        &self.conn,
                        &[evicted.id],
                        "rejected",
                        None,
                        Some(&format!("{err:?}")),
                    )?;
                    if txs.is_empty() && deposits.is_empty() {
                        return Ok(None);
                    }
                }
            }
        }
    }

    fn record_proof(
        &mut self,
        batch_num: u64,
        proof: Vec<u8>,
        public_inputs: Vec<u8>,
    ) -> Result<String, ApiError> {
        let batch = db::get_batch(&self.conn, batch_num)?.ok_or(ApiError::NotFound)?;
        let mut expected = Vec::with_capacity(160);
        for word in [&batch.old_root, &batch.new_root] {
            expected.extend_from_slice(&word[..]);
        }
        // deposit_hash/withdraw_hash aren't stored as columns; reparse from envelope.
        // Cheaper: trust bb's public_inputs only if the roots + da match and
        // length is exactly 160 — the deposit/withdraw folds were computed by
        // the same build we persisted.
        if public_inputs.len() != 160
            || public_inputs[..32] != batch.old_root
            || public_inputs[32..64] != batch.new_root
            || public_inputs[128..160] != batch.da_commitment
        {
            return Err(ApiError::Internal("bb public_inputs mismatch".into()));
        }
        if proof.len() != 14_592 {
            return Err(ApiError::Internal(format!("bad proof length {}", proof.len())));
        }
        let envelope: serde_json::Value = serde_json::from_str(&batch.envelope_json)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let mut envelope = envelope;
        envelope["proof"] = serde_json::Value::String(hex::encode(&proof));
        let envelope_json = envelope.to_string();
        db::batch_set_proof(&self.conn, batch_num, &proof, &envelope_json)?;
        tracing::info!(batch_num, "proof recorded");
        Ok(envelope_json)
    }

    fn mark_submitting(&mut self, batch_num: u64) -> Result<String, ApiError> {
        let batch = db::get_batch(&self.conn, batch_num)?.ok_or(ApiError::NotFound)?;
        if batch.proof.is_none() {
            return Err(ApiError::Internal("no proof recorded".into()));
        }
        db::batch_set_status(&self.conn, batch_num, "submitting")?;
        Ok(batch.envelope_json)
    }

    /// Apply a landed batch to the live tree + leaves table by replaying the
    /// stored blob through the same build path — one code path for build and
    /// apply means the two can't diverge.
    fn confirm_batch(&mut self, batch_num: u64) -> Result<(), ApiError> {
        let hasher = Hasher::new();
        let batch = db::get_batch(&self.conn, batch_num)?.ok_or(ApiError::NotFound)?;
        let blob: serde_json::Value = serde_json::from_str(&batch.blob_json)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (deposits, txs) = parse_blob(&blob).map_err(ApiError::Internal)?;
        let witness = build_batch(
            &hasher,
            &mut self.tree,
            self.cfg.deposit_slots,
            self.cfg.tx_slots,
            &deposits,
            &txs,
        )
        .map_err(|e| ApiError::Internal(format!("replay failed: {e:?}")))?;
        if witness.new_root != batch.new_root {
            return Err(ApiError::Internal("replay root mismatch".into()));
        }

        // Persist everything atomically.
        let tx = self.conn.unchecked_transaction()?;
        for (idx, account) in self.tree.leaves.iter() {
            db::upsert_leaf(&tx, *idx, &account.pk_x, account.balance, account.nonce)?;
        }
        db::meta_set(&tx, "confirmed_batch_num", &batch_num.to_string())?;

        // History + terminal statuses.
        let dep_seqs: Vec<u64> = blob["deposits"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|d| d["seq"].as_u64().unwrap_or(0))
            .collect();
        db::deposits_set_status(&tx, &dep_seqs, "consumed", Some(batch_num))?;
        for d in &deposits {
            db::insert_history(&tx, &d.pk_x, batch_num, "deposit", None, d.amount, None)?;
        }
        let tx_ids: Vec<i64> = blob["txs"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|t| t["mempool_id"].as_i64())
            .collect();
        db::mempool_set_status(&tx, &tx_ids, "included", Some(batch_num), None)?;
        for (i, t) in txs.iter().enumerate() {
            let dest = blob["txs"][i]["withdraw_dest"].as_str().map(String::from);
            if t.is_withdraw {
                db::insert_history(&tx, &t.from_pk_x, batch_num, "withdraw", dest.as_deref(), t.amount, Some(t.nonce))?;
            } else {
                db::insert_history(&tx, &t.from_pk_x, batch_num, "transfer_out", Some(&fr_hex(&t.to_field)), t.amount, Some(t.nonce))?;
                db::insert_history(&tx, &t.to_field, batch_num, "transfer_in", Some(&fr_hex(&t.from_pk_x)), t.amount, None)?;
            }
        }
        db::batch_set_status(&tx, batch_num, "confirmed")?;
        tx.commit()?;
        tracing::info!(batch_num, root = %fr_hex(&batch.new_root), "batch confirmed");
        Ok(())
    }

    fn fail_batch(&mut self, batch_num: u64, reason: &str) -> Result<(), ApiError> {
        tracing::warn!(batch_num, reason, "batch failed; requeueing inputs");
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE mempool SET status = 'pending', batch_num = NULL WHERE batch_num = ?1 AND status = 'batching'",
            [batch_num as i64],
        )?;
        tx.execute(
            "UPDATE deposits SET status = 'pending', batch_num = NULL WHERE batch_num = ?1 AND status = 'batching'",
            [batch_num as i64],
        )?;
        db::batch_set_status(&tx, batch_num, "failed")?;
        tx.commit()?;
        Ok(())
    }

    fn clone_tree(&self) -> Tree {
        Tree { leaves: self.tree.leaves.clone() }
    }

}

/// The published DA blob for one batch (free function: also the seam the
/// round-trip tests drive — `parse_blob(blob_json(..)) == build inputs`).
pub(crate) fn blob_json(
    batch_num: u64,
    witness: &harness::batch::BatchWitness,
    deposits: &[db::DepositRow],
    txs: &[db::MempoolRow],
) -> String {
        serde_json::json!({
            "batch_num": batch_num,
            "old_root": fr_hex(&witness.old_root),
            "new_root": fr_hex(&witness.new_root),
            "deposit_count": deposits.len(),
            "deposits": deposits.iter().map(|d| serde_json::json!({
                "seq": d.seq, "pk_x": fr_hex(&d.pk_x), "amount": d.amount.to_string(),
            })).collect::<Vec<_>>(),
            "withdrawals": txs.iter().filter(|t| t.is_withdraw).map(|t| serde_json::json!({
                "dest": t.withdraw_dest, "amount": t.amount.to_string(),
            })).collect::<Vec<_>>(),
            "da_commitment": fr_hex(&witness.da_commitment),
            "txs": txs.iter().map(|t| serde_json::json!({
                "mempool_id": t.id,
                "from_pk_x": fr_hex(&t.from_pk_x),
                "from_pk_y": fr_hex(&t.from_pk_y),
                "to_field": fr_hex(&t.to_field),
                "withdraw_dest": t.withdraw_dest,
                "amount": t.amount.to_string(),
                "nonce": t.nonce,
                "is_withdraw": t.is_withdraw,
                "sig": {
                    "r_x": fr_hex(&t.sig_r_x), "r_y": fr_hex(&t.sig_r_y),
                    "s_lo": fr_hex(&t.sig_s_lo), "s_hi": fr_hex(&t.sig_s_hi),
                },
            })).collect::<Vec<_>>(),
        })
        .to_string()
}

fn row_to_signed(row: &db::MempoolRow) -> SignedTx {
    SignedTx {
        from_pk_x: row.from_pk_x,
        from_pk_y: row.from_pk_y,
        to_field: row.to_field,
        amount: row.amount,
        nonce: row.nonce,
        is_withdraw: row.is_withdraw,
        sig: Signature::from_limbs(row.sig_r_x, row.sig_r_y, row.sig_s_lo, row.sig_s_hi)
            .expect("db sig corrupt"),
    }
}

/// CLI-arg-ready envelope; `proof` filled after proving. BytesN fields are
/// bare hex (no 0x) per the stellar CLI's JSON arg convention.
fn envelope_json(
    witness: &harness::batch::BatchWitness,
    deposit_count: u32,
    txs: &[db::MempoolRow],
) -> String {
    let strip = |fr: &Fr| hex::encode(fr);
    serde_json::json!({
        "new_root": strip(&witness.new_root),
        "deposit_count": deposit_count,
        "withdrawals": txs.iter().filter(|t| t.is_withdraw).map(|t| serde_json::json!({
            "dest": t.withdraw_dest, "amount": t.amount.to_string(),
        })).collect::<Vec<_>>(),
        "da_commitment": strip(&witness.da_commitment),
        "proof": "",
    })
    .to_string()
}

/// Reconstruct build inputs from a stored DA blob (also the documented
/// external-verifier recipe: re-fold tx messages -> da_commitment).
pub fn parse_blob(blob: &serde_json::Value) -> Result<(Vec<DepositRequest>, Vec<SignedTx>), String> {
    let mut deposits = Vec::new();
    for d in blob["deposits"].as_array().ok_or("bad blob: deposits")? {
        deposits.push(DepositRequest {
            pk_x: parse_fr(d["pk_x"].as_str().ok_or("bad pk_x")?).map_err(|e| format!("{e:?}"))?,
            amount: d["amount"].as_str().ok_or("bad amount")?.parse().map_err(|_| "bad amount")?,
        });
    }
    let mut txs = Vec::new();
    for t in blob["txs"].as_array().ok_or("bad blob: txs")? {
        let fr = |key: &str| -> Result<Fr, String> {
            parse_fr(t[key].as_str().ok_or_else(|| format!("bad {key}"))?)
                .map_err(|e| format!("{key}: {e:?}"))
        };
        let sig_fr = |key: &str| -> Result<Fr, String> {
            parse_fr(t["sig"][key].as_str().ok_or_else(|| format!("bad sig.{key}"))?)
                .map_err(|e| format!("sig.{key}: {e:?}"))
        };
        txs.push(SignedTx {
            from_pk_x: fr("from_pk_x")?,
            from_pk_y: fr("from_pk_y")?,
            to_field: fr("to_field")?,
            amount: t["amount"].as_str().ok_or("bad amount")?.parse().map_err(|_| "bad amount")?,
            nonce: t["nonce"].as_u64().ok_or("bad nonce")?,
            is_withdraw: t["is_withdraw"].as_bool().ok_or("bad is_withdraw")?,
            sig: Signature::from_limbs(sig_fr("r_x")?, sig_fr("r_y")?, sig_fr("s_lo")?, sig_fr("s_hi")?)
                .ok_or("bad sig limbs")?,
        });
    }
    Ok((deposits, txs))
}

/// Boot-time state loading + reconciliation against the chain.
pub struct BootState {
    pub tree: Tree,
    pub chain_synced: bool,
}

pub fn load_and_reconcile(
    conn: &Connection,
    chain_root: &Fr,
    chain_batch_num: u64,
) -> Result<BootState, String> {
    let hasher = Hasher::new();
    let mut tree = Tree::new();
    for (idx, pk_x, balance, nonce) in db::load_leaves(conn).map_err(|e| e.to_string())? {
        tree.set(idx, Account { pk_x, balance, nonce });
    }
    let db_batch_num = db::meta_get_u64(conn, "confirmed_batch_num").map_err(|e| e.to_string())?;
    let inflight = db::inflight_batch(conn).map_err(|e| e.to_string())?;

    // Case: crashed after landing, before recording -> finish the confirm by
    // replaying the stored blob (main() routes this through the engine once
    // spawned; here we just classify).
    if chain_batch_num == db_batch_num + 1 {
        if let Some(batch) = &inflight {
            if batch.new_root == *chain_root {
                tracing::warn!(batch.batch_num, "recovering: batch landed before crash");
                // Confirmation happens via the engine after spawn (needs &mut tree);
                // signal by leaving status as-is; batcher resumes it.
                return Ok(BootState { tree, chain_synced: true });
            }
        }
        return Err(format!(
            "chain at batch {chain_batch_num} but DB at {db_batch_num} with no matching inflight — manual repair needed"
        ));
    }

    if chain_batch_num != db_batch_num {
        return Err(format!(
            "chain batch_num {chain_batch_num} != DB {db_batch_num} — someone else advanced the root? halting"
        ));
    }

    let local_root = tree.root(&hasher);
    if local_root != *chain_root {
        return Err(format!(
            "tree root {} != chain root {} — refusing to batch",
            fr_hex(&local_root),
            fr_hex(chain_root)
        ));
    }

    // Normalize a pre-submission inflight: building/proving work is lost on
    // crash (Prover.toml/bb output gone) — fail + requeue; proved/submitting/
    // submitted are resumable (proof is in the DB, resubmission is safe: the
    // proof binds old_root, double-landing is impossible).
    if let Some(batch) = inflight {
        if batch.status == "proving" {
            tracing::warn!(batch.batch_num, "recovering: failing interrupted prove");
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE mempool SET status = 'pending', batch_num = NULL WHERE batch_num = ?1 AND status = 'batching'",
                [batch.batch_num as i64],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE deposits SET status = 'pending', batch_num = NULL WHERE batch_num = ?1 AND status = 'batching'",
                [batch.batch_num as i64],
            )
            .map_err(|e| e.to_string())?;
            db::batch_set_status(&tx, batch.batch_num, "failed").map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
    }

    Ok(BootState { tree, chain_synced: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness::batch::{build_batch, tx_message, DepositRequest};
    use harness::keys::{sign_with_nonce, Keypair};
    use harness::poseidon::{fr_from_u64, FR_ZERO};

    /// parse_blob(blob_json(..)) reproduces the exact build inputs, and
    /// re-folding the blob's tx list reproduces the proven da_commitment —
    /// the documented external DA-verifier recipe, as a unit test.
    #[test]
    fn blob_round_trips_and_refolds() {
        let hasher = Hasher::new();
        let alice = Keypair::from_sk(ark_grumpkin::Fr::from(101u64));
        let bob = Keypair::from_sk(ark_grumpkin::Fr::from(202u64));

        let deposits = vec![
            DepositRequest { pk_x: alice.pk_x(), amount: 1_000_000 },
            DepositRequest { pk_x: bob.pk_x(), amount: 500_000 },
        ];
        let msg1 = tx_message(&hasher, alice.pk_x(), bob.pk_x(), 250_000, 0, false);
        let sig1 = sign_with_nonce(&hasher, &alice, msg1, ark_grumpkin::Fr::from(41u64));
        let wd_field = fr_from_u64(770_007);
        let msg2 = tx_message(&hasher, bob.pk_x(), wd_field, 100_000, 0, true);
        let sig2 = sign_with_nonce(&hasher, &bob, msg2, ark_grumpkin::Fr::from(42u64));

        let signed = vec![
            SignedTx {
                from_pk_x: alice.pk_x(), from_pk_y: alice.pk_y(), to_field: bob.pk_x(),
                amount: 250_000, nonce: 0, is_withdraw: false, sig: sig1.clone(),
            },
            SignedTx {
                from_pk_x: bob.pk_x(), from_pk_y: bob.pk_y(), to_field: wd_field,
                amount: 100_000, nonce: 0, is_withdraw: true, sig: sig2.clone(),
            },
        ];
        let mut tree = Tree::new();
        let witness = build_batch(&hasher, &mut tree, 2, 4, &deposits, &signed).unwrap();

        // Rows as the DB would hold them.
        let dep_rows: Vec<db::DepositRow> = deposits
            .iter()
            .enumerate()
            .map(|(i, d)| db::DepositRow { seq: i as u64, pk_x: d.pk_x, amount: d.amount, status: "batching".into() })
            .collect();
        let tx_rows: Vec<db::MempoolRow> = signed
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (s_lo, s_hi) = t.sig.s_limbs();
                db::MempoolRow {
                    id: i as i64 + 1,
                    from_pk_x: t.from_pk_x,
                    from_pk_y: t.from_pk_y,
                    to_field: t.to_field,
                    withdraw_dest: t.is_withdraw.then(|| "GTESTDEST".to_string()),
                    amount: t.amount,
                    nonce: t.nonce,
                    is_withdraw: t.is_withdraw,
                    sig_r_x: t.sig.r_x,
                    sig_r_y: t.sig.r_y,
                    sig_s_lo: s_lo,
                    sig_s_hi: s_hi,
                    status: "batching".into(),
                    received_at: 0,
                }
            })
            .collect();

        let blob_str = blob_json(7, &witness, &dep_rows, &tx_rows);
        let blob: serde_json::Value = serde_json::from_str(&blob_str).unwrap();
        assert_eq!(blob["batch_num"], 7);
        assert_eq!(blob["da_commitment"], fr_hex(&witness.da_commitment));

        // Round-trip to build inputs.
        let (deps2, txs2) = parse_blob(&blob).unwrap();
        assert_eq!(deps2.len(), deposits.len());
        for (a, b) in deposits.iter().zip(&deps2) {
            assert_eq!(a.pk_x, b.pk_x);
            assert_eq!(a.amount, b.amount);
        }
        assert_eq!(txs2.len(), signed.len());
        for (a, b) in signed.iter().zip(&txs2) {
            assert_eq!(a.from_pk_x, b.from_pk_x);
            assert_eq!(a.to_field, b.to_field);
            assert_eq!(a.amount, b.amount);
            assert_eq!(a.nonce, b.nonce);
            assert_eq!(a.is_withdraw, b.is_withdraw);
            assert_eq!(a.sig.s, b.sig.s);
        }

        // Rebuilding from the parsed blob reproduces the same witness
        // (deterministic replay — the confirm path's core assumption).
        let mut tree2 = Tree::new();
        let replay = build_batch(&hasher, &mut tree2, 2, 4, &deps2, &txs2).unwrap();
        assert_eq!(replay.new_root, witness.new_root);
        assert_eq!(replay.da_commitment, witness.da_commitment);

        // External verifier recipe: fold the blob's tx messages.
        let mut acc = FR_ZERO;
        for t in blob["txs"].as_array().unwrap() {
            let fr = |k: &str| parse_fr(t[k].as_str().unwrap()).unwrap();
            let msg = tx_message(
                &hasher,
                fr("from_pk_x"),
                fr("to_field"),
                t["amount"].as_str().unwrap().parse().unwrap(),
                t["nonce"].as_u64().unwrap(),
                t["is_withdraw"].as_bool().unwrap(),
            );
            acc = hasher.hash(&[fr_from_u64(harness::batch::DOMAIN_DA), acc, msg]);
        }
        assert_eq!(fr_hex(&acc), fr_hex(&witness.da_commitment));
    }
}
