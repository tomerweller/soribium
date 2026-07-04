import { useHistory } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { shortHex, stroopsToXlm } from '../format';
import { StatusBadge } from '../components/common';
import { Onboarding } from './Onboarding';

const KIND_LABEL: Record<string, string> = {
  deposit: 'Deposit',
  transfer_in: 'Received',
  transfer_out: 'Sent',
  withdraw: 'Withdrawal',
};

export function History() {
  const { wallet } = useKey();
  const { data, isLoading } = useHistory(wallet?.pkX);
  if (!wallet) return <Onboarding />;

  const entries = data?.entries ?? [];
  return (
    <div className="panel">
      <h2>History</h2>
      {isLoading && <p className="muted">Loading…</p>}
      {!isLoading && entries.length === 0 && <p className="muted">No activity yet.</p>}
      {entries.length > 0 && (
        <table>
          <thead>
            <tr><th>Type</th><th>Counterparty</th><th>Amount</th><th>Status</th></tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr key={`${e.status}-${e.id}`}>
                <td>{KIND_LABEL[e.kind] ?? e.kind}</td>
                <td className="mono">{e.counterparty ? shortHex(e.counterparty, 6) : '—'}</td>
                <td>{stroopsToXlm(BigInt(e.amount))} XLM</td>
                <td><StatusBadge status={e.status} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
