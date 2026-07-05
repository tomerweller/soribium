import { useEffect, useState } from 'react';
import { useParams, useStatus, useBatches } from '../api/queries';
import { readOnchainRoot } from '../api/stellar';
import { SEQUENCER_URL, contractUrl } from '../config';
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
  }>;

  return (
    <div className="panel">
      <h2>Rollup explorer</h2>
      <p className="muted" style={{ marginTop: '-0.4rem' }}>
        The validity proof: the sequencer's state root, checked against the contract on-chain.
      </p>
      {status && (
        <>
          <div className="row"><span className="muted">Batch</span><span>#{status.batch_num}</span></div>
          <div className="row"><span className="muted">Pending txs / deposits</span><span>{status.pending_txs} / {status.pending_deposits}</span></div>
          <div className="row"><span className="muted">Chain synced</span><Badge ok={status.chain_synced}>{String(status.chain_synced)}</Badge></div>
          {params && (
            <div className="row">
              <span className="muted">Contract</span>
              <CopyableHex value={params.contract_id} chars={6} />
            </div>
          )}
          <div className="row" style={{ marginTop: '0.75rem' }}>
            <span className="muted">Sequencer root</span><CopyableHex value={status.root} chars={8} />
          </div>
          <div className="row">
            <span className="muted">On-chain root</span>
            {chainRoot === undefined ? <span className="muted">…</span>
              : chainRoot === null ? <span className="muted">unavailable</span>
              : <CopyableHex value={chainRoot} chars={8} />}
          </div>
          {chainRoot != null && (
            <div className="row">
              <span className="muted">Roots match</span>
              <Badge ok={chainRoot === status.root}>{String(chainRoot === status.root)}</Badge>
            </div>
          )}
          {params && (
            <p style={{ marginTop: '0.75rem' }}>
              <a href={contractUrl(params.contract_id)} target="_blank" rel="noreferrer">
                Contract on stellar.expert ↗
              </a>
            </p>
          )}
        </>
      )}

      <h3>Batches</h3>
      <table>
        <thead><tr><th>#</th><th>New root</th><th>Status</th><th>DA blob</th></tr></thead>
        <tbody>
          {batches.map((b) => (
            <tr key={b.batch_num}>
              <td>{b.batch_num}</td>
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
