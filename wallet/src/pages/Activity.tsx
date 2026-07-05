import { useHistory } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { stroopsToXlm } from '../format';
import { CopyableHex, StatusBadge } from '../components/common';
import { Onboarding } from './Onboarding';

const KIND_LABEL: Record<string, string> = {
  deposit: 'Deposit',
  transfer_in: 'Received',
  transfer_out: 'Sent',
  withdraw: 'Withdrawal',
};

export function Activity() {
  const { wallet } = useKey();
  const { data, isLoading } = useHistory(wallet?.pkX);
  if (!wallet) return <Onboarding />;

  const entries = data?.entries ?? [];
  return (
    <div className="panel">
      <h2>Activity</h2>
      {isLoading && <p className="muted">Loading…</p>}
      {!isLoading && entries.length === 0 && <p className="muted">No activity yet.</p>}
      {entries.map((e) => {
        const incoming = e.kind === 'transfer_in' || e.kind === 'deposit';
        return (
          <div className="list-row" key={`${e.status}-${e.id}`}>
            <div className="who">
              <span className="kind">{KIND_LABEL[e.kind] ?? e.kind}</span>
              {e.counterparty && <span className="cp"><CopyableHex value={e.counterparty} chars={6} /></span>}
            </div>
            <div className="row" style={{ gap: '0.6rem' }}>
              <span className={incoming ? 'amt-in' : 'amt-out'}>
                {incoming ? '+' : '−'}{stroopsToXlm(BigInt(e.amount))} XLM
              </span>
              <StatusBadge status={e.status} />
            </div>
          </div>
        );
      })}
    </div>
  );
}
