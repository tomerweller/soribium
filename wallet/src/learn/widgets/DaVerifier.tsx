// S6 finale: fetch a REAL batch blob from the live deployment and re-fold
// its transaction messages with the real Poseidon2, comparing against the
// da_commitment the on-chain proof bound. The reader's browser audits the
// production system.
import { useState } from 'react';
import { api } from '../../api/sequencer';
import { CopyableHex } from '../../components/common';
import { frToHex32, hexToFr } from '../../crypto/fields';
import { daFold, txMessage } from '../../crypto/schnorr';

interface BlobTx {
  from_pk_x: string;
  to_field: string;
  amount: string;
  nonce: number;
  is_withdraw: boolean;
}

type Phase =
  | { s: 'idle' }
  | { s: 'error'; msg: string }
  | { s: 'done'; batch: number; txs: number; folded: string; committed: string; match: boolean };

export function DaVerifier() {
  const [phase, setPhase] = useState<Phase>({ s: 'idle' });
  const [busy, setBusy] = useState(false);

  async function run() {
    setBusy(true);
    try {
      // Find the latest confirmed batch with transactions.
      const { batches } = (await api.batches()) as { batches: Array<{ batch_num: number; status: string }> };
      const confirmed = batches.filter((b) => b.status === 'confirmed').map((b) => b.batch_num).sort((a, b) => b - a);
      let blob: Record<string, unknown> | null = null;
      for (const n of confirmed) {
        const candidate = await api.da(n);
        if (((candidate.txs as BlobTx[]) ?? []).length > 0) {
          blob = candidate;
          break;
        }
      }
      if (!blob) throw new Error('no confirmed batch with transactions yet — make a payment in the wallet first');

      const txs = blob.txs as BlobTx[];
      // The verifier recipe from DESIGN.md, byte for byte.
      let acc = 0n;
      for (const t of txs) {
        const msg = txMessage(hexToFr(t.from_pk_x), hexToFr(t.to_field), BigInt(t.amount), BigInt(t.nonce), t.is_withdraw);
        acc = daFold(acc, msg);
      }
      const folded = frToHex32(acc);
      const committed = blob.da_commitment as string;
      setPhase({
        s: 'done',
        batch: blob.batch_num as number,
        txs: txs.length,
        folded,
        committed,
        match: folded === committed,
      });
    } catch (e) {
      setPhase({ s: 'error', msg: e instanceof Error ? e.message : String(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="learn-widget">
      <button className="primary" onClick={run} disabled={busy}>
        {busy ? 'fetching + folding…' : 'Audit the live deployment'}
      </button>
      {phase.s === 'error' && (
        <p className="muted" style={{ fontSize: '0.78rem' }}>
          Live instance unreachable ({phase.msg}). The recipe still stands: fetch{' '}
          <span className="mono">/da/:batch</span>, fold each tx message with
          Poseidon2([7, acc, msg]), compare to the proven commitment.
        </p>
      )}
      {phase.s === 'done' && (
        <>
          <div className="kv"><span className="k">Batch · txs</span><span className="dots" /><span className="v">#{phase.batch} · {phase.txs} txs</span></div>
          <div className="kv"><span className="k">Your browser's fold</span><span className="dots" /><span className="v"><CopyableHex value={phase.folded} chars={8} /></span></div>
          <div className="kv"><span className="k">Proven on-chain</span><span className="dots" /><span className="v"><CopyableHex value={phase.committed} chars={8} /></span></div>
          <div className={`verdict ${phase.match ? 'verdict-ok' : 'verdict-bad'}`}>
            <span className="verdict-glyph">{phase.match ? '▣' : '▨'}</span>
            {phase.match ? 'MATCH — YOUR BROWSER JUST AUDITED THE LIVE SYSTEM' : 'MISMATCH — THE OPERATOR PUBLISHED WRONG DATA'}
          </div>
        </>
      )}
    </div>
  );
}
