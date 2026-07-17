import { useEffect, useState } from 'react';
import { useParams, useStatus, useBatches } from '../api/queries';
import { readOnchainRoot } from '../api/stellar';
import { SEQUENCER_URL, contractUrl } from '../config';
import { formatTs } from '../format';
import { Badge, CopyableHex } from '../components/common';

export function Explorer() {
  const { data: params } = useParams();
  const { data: status } = useStatus();
  const { data: batchesData } = useBatches();
  const [chainRoot, setChainRoot] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    if (params) readOnchainRoot(params).then(setChainRoot);
  }, [params, status?.batch_num]);

  const batches = (batchesData?.batches ?? []) as Array<{
    batch_num: number;
    new_root: string;
    status: string;
    tx_hash: string | null;
    created_at: number;
    confirmed_at: number | null;
  }>;

  return (
    <div className="panel">
      <h2>Rollup explorer</h2>
      <p className="muted" style={{ marginTop: '-0.4rem' }}>
        The validity proof: the sequencer's state root, checked against the contract on-chain.
      </p>
      {status && (
        <>
          <div className="kv"><span className="k">Batch</span><span className="dots" /><span className="v">#{status.batch_num}</span></div>
          <div className="kv"><span className="k">Pending tx / dep</span><span className="dots" /><span className="v">{status.pending_txs} / {status.pending_deposits}</span></div>
          <div className="kv"><span className="k">Chain synced</span><span className="dots" /><span className="v"><Badge ok={status.chain_synced}>{String(status.chain_synced)}</Badge></span></div>
          {params && (
            <div className="kv"><span className="k">Contract</span><span className="dots" /><span className="v"><CopyableHex value={params.contract_id} chars={6} /></span></div>
          )}
          <div className="kv"><span className="k">Sequencer root</span><span className="dots" /><span className="v"><CopyableHex value={status.root} chars={8} /></span></div>
          <div className="kv">
            <span className="k">On-chain root</span><span className="dots" />
            <span className="v">
              {chainRoot === undefined ? <span className="muted">…</span>
                : chainRoot === null ? <span className="muted">unavailable</span>
                : <CopyableHex value={chainRoot} chars={8} />}
            </span>
          </div>
          {chainRoot != null && (
            <div className={`verdict ${chainRoot === status.root ? 'verdict-ok' : 'verdict-bad'}`}>
              <span className="verdict-glyph">{chainRoot === status.root ? '▣' : '▨'}</span>
              {chainRoot === status.root ? 'ROOTS MATCH — STATE PROVEN ON-CHAIN' : 'ROOT MISMATCH'}
            </div>
          )}
          {params && (
            <p style={{ marginTop: '0.9rem' }}>
              <a href={contractUrl(params.contract_id)} target="_blank" rel="noreferrer">
                Contract on stellar.expert ↗
              </a>
            </p>
          )}
        </>
      )}

      <h3>Batches</h3>
      <table>
        <thead><tr><th>#</th><th>Time</th><th>New root</th><th>Status</th><th>DA blob</th></tr></thead>
        <tbody>
          {batches.map((b) => (
            <tr key={b.batch_num}>
              <td>{b.batch_num}</td>
              {/* Confirmation time once on-chain; build time (muted) until then.
                  Falls back to a dash against sequencers predating these fields. */}
              <td className={b.confirmed_at ? undefined : 'muted'}>
                {b.confirmed_at ?? b.created_at ? formatTs((b.confirmed_at ?? b.created_at)!) : '—'}
              </td>
              <td><CopyableHex value={b.new_root} chars={6} /></td>
              <td>{b.status}</td>
              <td><a href={`${SEQUENCER_URL}/da/${b.batch_num}`} target="_blank" rel="noreferrer">download</a></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
