//! SQLite persistence. Conventions: Fr values as strict 0x-hex (hexutil),
//! amounts as decimal TEXT (u64 range exceeds SQLite's i64), timestamps as
//! unix seconds. WAL + synchronous=FULL: rows written before irreversible
//! actions (proving, submitting) are the crash-recovery ground truth.

use crate::hexutil::{fr_hex, parse_fr};
use harness::poseidon::Fr;
use rusqlite::{params, Connection, OptionalExtension};

pub type DbResult<T> = Result<T, rusqlite::Error>;

pub const SCHEMA_VERSION: i64 = 1;

pub fn open(path: &std::path::Path) -> DbResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> DbResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        BEGIN;
        CREATE TABLE IF NOT EXISTS meta (
          key   TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS leaves (
          idx     INTEGER PRIMARY KEY,
          pk_x    TEXT NOT NULL UNIQUE,
          balance TEXT NOT NULL,
          nonce   INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS mempool (
          id            INTEGER PRIMARY KEY AUTOINCREMENT,
          from_pk_x     TEXT NOT NULL,
          from_pk_y     TEXT NOT NULL,
          to_field      TEXT NOT NULL,
          withdraw_dest TEXT,
          amount        TEXT NOT NULL,
          nonce         INTEGER NOT NULL,
          is_withdraw   INTEGER NOT NULL,
          sig_r_x  TEXT NOT NULL,
          sig_r_y  TEXT NOT NULL,
          sig_s_lo TEXT NOT NULL,
          sig_s_hi TEXT NOT NULL,
          status      TEXT NOT NULL DEFAULT 'pending',
          batch_num   INTEGER,
          reject_reason TEXT,
          received_at INTEGER NOT NULL,
          UNIQUE(from_pk_x, nonce)
        );
        CREATE TABLE IF NOT EXISTS deposits (
          seq         INTEGER PRIMARY KEY,
          pk_x        TEXT NOT NULL,
          amount      TEXT NOT NULL,
          status      TEXT NOT NULL DEFAULT 'pending',
          batch_num   INTEGER,
          observed_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS batches (
          batch_num     INTEGER PRIMARY KEY,
          old_root      TEXT NOT NULL,
          new_root      TEXT NOT NULL,
          deposit_count INTEGER NOT NULL,
          da_commitment TEXT NOT NULL,
          blob_json     TEXT NOT NULL,
          envelope_json TEXT NOT NULL,
          proof         BLOB,
          status  TEXT NOT NULL,
          tx_hash TEXT,
          created_at INTEGER NOT NULL,
          confirmed_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS history (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          pk_x TEXT NOT NULL,
          batch_num INTEGER NOT NULL,
          kind TEXT NOT NULL,
          counterparty TEXT,
          amount TEXT NOT NULL,
          nonce INTEGER,
          ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS history_pk ON history(pk_x, id DESC);
        PRAGMA user_version = 1;
        COMMIT;
        "#,
    )
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// ---------- meta ----------

pub fn meta_get(conn: &Connection, key: &str) -> DbResult<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn meta_set(conn: &Connection, key: &str, value: &str) -> DbResult<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn meta_get_u64(conn: &Connection, key: &str) -> DbResult<u64> {
    Ok(meta_get(conn, key)?.and_then(|v| v.parse().ok()).unwrap_or(0))
}

// ---------- leaves ----------

pub fn load_leaves(conn: &Connection) -> DbResult<Vec<(u32, Fr, u64, u64)>> {
    let mut stmt = conn.prepare("SELECT idx, pk_x, balance, nonce FROM leaves")?;
    let rows = stmt.query_map([], |r| {
        let idx: u32 = r.get(0)?;
        let pk_x: String = r.get(1)?;
        let balance: String = r.get(2)?;
        let nonce: i64 = r.get(3)?;
        Ok((idx, pk_x, balance, nonce))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (idx, pk_x, balance, nonce) = row?;
        out.push((
            idx,
            parse_fr(&pk_x).expect("db pk_x corrupt"),
            balance.parse().expect("db balance corrupt"),
            nonce as u64,
        ));
    }
    Ok(out)
}

pub fn upsert_leaf(conn: &Connection, idx: u32, pk_x: &Fr, balance: u64, nonce: u64) -> DbResult<()> {
    conn.execute(
        "INSERT INTO leaves(idx, pk_x, balance, nonce) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(idx) DO UPDATE SET pk_x = excluded.pk_x,
           balance = excluded.balance, nonce = excluded.nonce",
        params![idx, fr_hex(pk_x), balance.to_string(), nonce as i64],
    )?;
    Ok(())
}

// ---------- mempool ----------

#[derive(Debug, Clone)]
#[allow(dead_code)] // status/received_at mirror DB columns; not all read yet
pub struct MempoolRow {
    pub id: i64,
    pub from_pk_x: Fr,
    pub from_pk_y: Fr,
    pub to_field: Fr,
    pub withdraw_dest: Option<String>,
    pub amount: u64,
    pub nonce: u64,
    pub is_withdraw: bool,
    pub sig_r_x: Fr,
    pub sig_r_y: Fr,
    pub sig_s_lo: Fr,
    pub sig_s_hi: Fr,
    pub status: String,
    pub received_at: i64,
}

fn mempool_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MempoolRow> {
    let get_fr = |i: usize| -> rusqlite::Result<Fr> {
        let s: String = r.get(i)?;
        Ok(parse_fr(&s).expect("db fr corrupt"))
    };
    Ok(MempoolRow {
        id: r.get(0)?,
        from_pk_x: get_fr(1)?,
        from_pk_y: get_fr(2)?,
        to_field: get_fr(3)?,
        withdraw_dest: r.get(4)?,
        amount: r.get::<_, String>(5)?.parse().expect("db amount corrupt"),
        nonce: r.get::<_, i64>(6)? as u64,
        is_withdraw: r.get::<_, i64>(7)? != 0,
        sig_r_x: get_fr(8)?,
        sig_r_y: get_fr(9)?,
        sig_s_lo: get_fr(10)?,
        sig_s_hi: get_fr(11)?,
        status: r.get(12)?,
        received_at: r.get(13)?,
    })
}

const MEMPOOL_COLS: &str = "id, from_pk_x, from_pk_y, to_field, withdraw_dest, amount, nonce, \
                            is_withdraw, sig_r_x, sig_r_y, sig_s_lo, sig_s_hi, status, received_at";

#[allow(clippy::too_many_arguments)]
pub fn insert_mempool(
    conn: &Connection,
    from_pk_x: &Fr,
    from_pk_y: &Fr,
    to_field: &Fr,
    withdraw_dest: Option<&str>,
    amount: u64,
    nonce: u64,
    is_withdraw: bool,
    sig: [&Fr; 4],
) -> DbResult<i64> {
    conn.execute(
        "INSERT INTO mempool(from_pk_x, from_pk_y, to_field, withdraw_dest, amount, nonce,
                             is_withdraw, sig_r_x, sig_r_y, sig_s_lo, sig_s_hi, received_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            fr_hex(from_pk_x),
            fr_hex(from_pk_y),
            fr_hex(to_field),
            withdraw_dest,
            amount.to_string(),
            nonce as i64,
            is_withdraw as i64,
            fr_hex(sig[0]),
            fr_hex(sig[1]),
            fr_hex(sig[2]),
            fr_hex(sig[3]),
            now(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn mempool_find(conn: &Connection, from_pk_x: &Fr, nonce: u64) -> DbResult<Option<(i64, String)>> {
    conn.query_row(
        "SELECT id, status FROM mempool WHERE from_pk_x = ?1 AND nonce = ?2",
        params![fr_hex(from_pk_x), nonce as i64],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

/// Pending txs in arrival order (per-sender nonce order falls out of the
/// UNIQUE(from,nonce) admission rule + arrival ordering).
pub fn mempool_pending(conn: &Connection, limit: usize) -> DbResult<Vec<MempoolRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {MEMPOOL_COLS} FROM mempool WHERE status = 'pending' ORDER BY id LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit as i64], |r| mempool_row(r))?;
    rows.collect()
}

pub fn mempool_pending_for(conn: &Connection, from_pk_x: &Fr) -> DbResult<Vec<MempoolRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {MEMPOOL_COLS} FROM mempool
         WHERE from_pk_x = ?1 AND status IN ('pending','batching') ORDER BY nonce"
    ))?;
    let rows = stmt.query_map([fr_hex(from_pk_x)], |r| mempool_row(r))?;
    rows.collect()
}

pub fn mempool_count_pending(conn: &Connection) -> DbResult<u64> {
    conn.query_row(
        "SELECT COUNT(*) FROM mempool WHERE status = 'pending'",
        [],
        |r| r.get::<_, i64>(0).map(|v| v as u64),
    )
}

pub fn mempool_oldest_pending_age(conn: &Connection) -> DbResult<Option<i64>> {
    conn.query_row(
        "SELECT MIN(received_at) FROM mempool WHERE status = 'pending'",
        [],
        |r| r.get::<_, Option<i64>>(0),
    )
    .map(|min| min.map(|m| now() - m))
}

pub fn mempool_set_status(conn: &Connection, ids: &[i64], status: &str, batch_num: Option<u64>, reason: Option<&str>) -> DbResult<()> {
    for id in ids {
        conn.execute(
            "UPDATE mempool SET status = ?1, batch_num = ?2, reject_reason = ?3 WHERE id = ?4",
            params![status, batch_num.map(|b| b as i64), reason, id],
        )?;
    }
    Ok(())
}

// ---------- deposits ----------

#[derive(Debug, Clone)]
#[allow(dead_code)] // status mirrors a DB column; not read by consumers yet
pub struct DepositRow {
    pub seq: u64,
    pub pk_x: Fr,
    pub amount: u64,
    pub status: String,
}

pub fn insert_deposit(conn: &Connection, seq: u64, pk_x: &Fr, amount: u64) -> DbResult<bool> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO deposits(seq, pk_x, amount, observed_at) VALUES (?1,?2,?3,?4)",
        params![seq as i64, fr_hex(pk_x), amount.to_string(), now()],
    )?;
    Ok(n > 0)
}

pub fn deposits_pending(conn: &Connection, limit: usize) -> DbResult<Vec<DepositRow>> {
    let mut stmt = conn.prepare(
        "SELECT seq, pk_x, amount, status FROM deposits WHERE status = 'pending' ORDER BY seq LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok(DepositRow {
            seq: r.get::<_, i64>(0)? as u64,
            pk_x: parse_fr(&r.get::<_, String>(1)?).expect("db pk_x corrupt"),
            amount: r.get::<_, String>(2)?.parse().expect("db amount corrupt"),
            status: r.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn deposits_count_pending(conn: &Connection) -> DbResult<u64> {
    conn.query_row("SELECT COUNT(*) FROM deposits WHERE status = 'pending'", [], |r| {
        r.get::<_, i64>(0).map(|v| v as u64)
    })
}

pub fn deposits_pending_pk(conn: &Connection, pk_x: &Fr) -> DbResult<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM deposits WHERE pk_x = ?1 AND status IN ('pending','batching')",
        [fr_hex(pk_x)],
        |r| r.get::<_, i64>(0).map(|v| v > 0),
    )
}

pub fn deposits_oldest_pending_age(conn: &Connection) -> DbResult<Option<i64>> {
    conn.query_row(
        "SELECT MIN(observed_at) FROM deposits WHERE status = 'pending'",
        [],
        |r| r.get::<_, Option<i64>>(0),
    )
    .map(|min| min.map(|m| now() - m))
}

pub fn deposits_set_status(conn: &Connection, seqs: &[u64], status: &str, batch_num: Option<u64>) -> DbResult<()> {
    for seq in seqs {
        conn.execute(
            "UPDATE deposits SET status = ?1, batch_num = ?2 WHERE seq = ?3",
            params![status, batch_num.map(|b| b as i64), *seq as i64],
        )?;
    }
    Ok(())
}

// ---------- batches ----------

#[derive(Debug, Clone)]
pub struct BatchRow {
    pub batch_num: u64,
    pub old_root: Fr,
    pub new_root: Fr,
    pub deposit_count: u32,
    pub da_commitment: Fr,
    pub blob_json: String,
    pub envelope_json: String,
    pub proof: Option<Vec<u8>>,
    pub status: String,
    pub tx_hash: Option<String>,
}

fn batch_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<BatchRow> {
    Ok(BatchRow {
        batch_num: r.get::<_, i64>(0)? as u64,
        old_root: parse_fr(&r.get::<_, String>(1)?).expect("db root corrupt"),
        new_root: parse_fr(&r.get::<_, String>(2)?).expect("db root corrupt"),
        deposit_count: r.get::<_, i64>(3)? as u32,
        da_commitment: parse_fr(&r.get::<_, String>(4)?).expect("db da corrupt"),
        blob_json: r.get(5)?,
        envelope_json: r.get(6)?,
        proof: r.get(7)?,
        status: r.get(8)?,
        tx_hash: r.get(9)?,
    })
}

const BATCH_COLS: &str = "batch_num, old_root, new_root, deposit_count, da_commitment, \
                          blob_json, envelope_json, proof, status, tx_hash";

#[allow(clippy::too_many_arguments)]
pub fn insert_batch(
    conn: &Connection,
    batch_num: u64,
    old_root: &Fr,
    new_root: &Fr,
    deposit_count: u32,
    da_commitment: &Fr,
    blob_json: &str,
    envelope_json: &str,
) -> DbResult<()> {
    conn.execute(
        "INSERT INTO batches(batch_num, old_root, new_root, deposit_count, da_commitment,
                             blob_json, envelope_json, status, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,'proving',?8)",
        params![
            batch_num as i64,
            fr_hex(old_root),
            fr_hex(new_root),
            deposit_count as i64,
            fr_hex(da_commitment),
            blob_json,
            envelope_json,
            now(),
        ],
    )?;
    Ok(())
}

/// Remove a failed batch row so its batch_num (PRIMARY KEY) can be reused by
/// the rebuild — a lingering 'failed' row would make the next insert_batch
/// hit the UNIQUE constraint and wedge batching permanently.
pub fn delete_batch(conn: &Connection, batch_num: u64) -> DbResult<()> {
    conn.execute("DELETE FROM batches WHERE batch_num = ?1", [batch_num as i64])?;
    Ok(())
}

pub fn get_batch(conn: &Connection, batch_num: u64) -> DbResult<Option<BatchRow>> {
    conn.query_row(
        &format!("SELECT {BATCH_COLS} FROM batches WHERE batch_num = ?1"),
        [batch_num as i64],
        |r| batch_row(r),
    )
    .optional()
}

/// The single non-terminal batch, if any.
pub fn inflight_batch(conn: &Connection) -> DbResult<Option<BatchRow>> {
    conn.query_row(
        &format!(
            "SELECT {BATCH_COLS} FROM batches WHERE status NOT IN ('confirmed','failed')
             ORDER BY batch_num DESC LIMIT 1"
        ),
        [],
        |r| batch_row(r),
    )
    .optional()
}

pub fn batch_set_status(conn: &Connection, batch_num: u64, status: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE batches SET status = ?1 WHERE batch_num = ?2",
        params![status, batch_num as i64],
    )?;
    Ok(())
}

pub fn batch_set_proof(conn: &Connection, batch_num: u64, proof: &[u8], envelope_json: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE batches SET proof = ?1, envelope_json = ?2, status = 'proved' WHERE batch_num = ?3",
        params![proof, envelope_json, batch_num as i64],
    )?;
    Ok(())
}

pub fn batch_set_submitted(conn: &Connection, batch_num: u64, tx_hash: Option<&str>) -> DbResult<()> {
    conn.execute(
        "UPDATE batches SET status = 'submitted', tx_hash = ?1 WHERE batch_num = ?2",
        params![tx_hash, batch_num as i64],
    )?;
    Ok(())
}

pub fn batch_list(conn: &Connection, limit: usize) -> DbResult<Vec<BatchRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {BATCH_COLS} FROM batches ORDER BY batch_num DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit as i64], |r| batch_row(r))?;
    rows.collect()
}

// ---------- history ----------

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub batch_num: Option<u64>,
    pub kind: String,
    pub counterparty: Option<String>,
    pub amount: String,
    pub nonce: Option<u64>,
    pub status: String,
    pub ts: i64,
}

pub fn insert_history(
    conn: &Connection,
    pk_x: &Fr,
    batch_num: u64,
    kind: &str,
    counterparty: Option<&str>,
    amount: u64,
    nonce: Option<u64>,
) -> DbResult<()> {
    conn.execute(
        "INSERT INTO history(pk_x, batch_num, kind, counterparty, amount, nonce, ts)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            fr_hex(pk_x),
            batch_num as i64,
            kind,
            counterparty,
            amount.to_string(),
            nonce.map(|n| n as i64),
            now(),
        ],
    )?;
    Ok(())
}

pub fn history_for(conn: &Connection, pk_x: &Fr, limit: usize) -> DbResult<Vec<HistoryEntry>> {
    let mut out = Vec::new();
    // Confirmed history.
    let mut stmt = conn.prepare(
        "SELECT id, batch_num, kind, counterparty, amount, nonce, ts FROM history
         WHERE pk_x = ?1 ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![fr_hex(pk_x), limit as i64], |r| {
        Ok(HistoryEntry {
            id: r.get(0)?,
            batch_num: Some(r.get::<_, i64>(1)? as u64),
            kind: r.get(2)?,
            counterparty: r.get(3)?,
            amount: r.get(4)?,
            nonce: r.get::<_, Option<i64>>(5)?.map(|n| n as u64),
            status: "batched".into(),
            ts: r.get(6)?,
        })
    })?;
    for row in rows {
        out.push(row?);
    }
    // Live mempool entries (pending/batching/rejected) for this sender.
    let mut stmt = conn.prepare(
        "SELECT id, to_field, withdraw_dest, amount, nonce, is_withdraw, status, reject_reason, received_at
         FROM mempool WHERE from_pk_x = ?1 AND status != 'included' ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([fr_hex(pk_x)], |r| {
        let is_withdraw: bool = r.get::<_, i64>(5)? != 0;
        let to_field: String = r.get(1)?;
        let withdraw_dest: Option<String> = r.get(2)?;
        let status: String = r.get(6)?;
        Ok(HistoryEntry {
            id: r.get(0)?,
            batch_num: None,
            kind: if is_withdraw { "withdraw".into() } else { "transfer_out".into() },
            counterparty: if is_withdraw { withdraw_dest } else { Some(to_field) },
            amount: r.get(3)?,
            nonce: Some(r.get::<_, i64>(4)? as u64),
            status: if status == "batching" { "pending".into() } else { status },
            ts: r.get(8)?,
        })
    })?;
    for row in rows {
        out.push(row?);
    }
    out.sort_by(|a, b| b.ts.cmp(&a.ts).then(b.id.cmp(&a.id)));
    Ok(out)
}
