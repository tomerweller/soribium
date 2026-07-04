import { useAccount } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { hexToFr } from '../crypto/fields';
import { leafValue, verifyPath } from '../crypto/merkle';
import { stroopsToXlm } from '../format';
import { Badge, ErrorText } from '../components/common';
import { Onboarding } from './Onboarding';

export function Balance() {
  const { wallet } = useKey();
  const { data: account, isLoading, error } = useAccount(wallet?.pkX);

  if (!wallet) return <Onboarding />;

  if (isLoading) return <div className="panel">Loading…</div>;
  if (error) return <div className="panel"><ErrorText error={error} /></div>;

  if (!account) {
    return (
      <div className="panel">
        <p className="muted">This account isn't on the rollup yet.</p>
        <p>Make a deposit to fund it — your balance appears after the next batch settles.</p>
      </div>
    );
  }

  // Verify the sequencer-returned Merkle inclusion locally against its root.
  const leaf = leafValue(
    hexToFr(account.pk_x),
    BigInt(account.balance),
    BigInt(account.nonce),
  );
  const included = verifyPath(
    leaf,
    account.index,
    account.siblings.map(hexToFr),
    hexToFr(account.root),
  );

  return (
    <div className="panel">
      <div className="balance-big">
        {stroopsToXlm(BigInt(account.balance))} <small>XLM</small>
      </div>
      <div className="row" style={{ marginTop: '1rem' }}>
        <span className="muted">nonce {account.nonce}</span>
        <Badge ok={included}>{included ? 'inclusion verified' : 'inclusion FAILED'}</Badge>
      </div>
      {account.pending_out !== '0' && (
        <p className="muted">Pending outgoing: {stroopsToXlm(BigInt(account.pending_out))} XLM</p>
      )}
      <p className="muted" style={{ marginTop: '1rem', fontSize: '0.8rem' }}>
        Verified against rollup root {account.root.slice(0, 10)}… at batch {account.batch_num}.
      </p>
    </div>
  );
}
