import { useNavigate } from 'react-router-dom';
import { useAccount, useHistory, usePending } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { hexToFr } from '../crypto/fields';
import { leafValue, verifyPath } from '../crypto/merkle';
import { shortHex, stroopsToXlm } from '../format';
import { Badge, ErrorText, StatusBadge, Tooltip } from '../components/common';
import { Onboarding } from './Onboarding';

const KIND_LABEL: Record<string, string> = {
  deposit: 'Deposit',
  transfer_in: 'Received',
  transfer_out: 'Sent',
  withdraw: 'Withdrawal',
};

export function Home() {
  const { wallet } = useKey();
  const navigate = useNavigate();
  const { data: account, isLoading, error } = useAccount(wallet?.pkX);
  const { data: history } = useHistory(wallet?.pkX);
  const pending = usePending(wallet?.pkX);

  if (!wallet) return <Onboarding />;

  const actions = (
    <div className="actions">
      <button className="action-btn" onClick={() => navigate('/send')}>
        <span className="ico">↑</span>Send
      </button>
      <button className="action-btn" onClick={() => navigate('/deposit')}>
        <span className="ico">↓</span>Deposit
      </button>
      <button className="action-btn" onClick={() => navigate('/receive')}>
        <span className="ico">＃</span>Receive
      </button>
    </div>
  );

  // Hero content depends on account state.
  let hero;
  if (isLoading) {
    hero = <div className="hero muted">Loading…</div>;
  } else if (error) {
    hero = <div className="hero"><ErrorText error={error} /></div>;
  } else if (!account) {
    hero = (
      <div className="hero">
        <div className="amount">0 <small>XLM</small></div>
        <p className="muted" style={{ marginTop: '0.75rem' }}>
          This account isn't on the rollup yet. Make a deposit to fund it.
        </p>
      </div>
    );
  } else {
    const leaf = leafValue(hexToFr(account.pk_x), BigInt(account.balance), BigInt(account.nonce));
    const included = verifyPath(leaf, account.index, account.siblings.map(hexToFr), hexToFr(account.root));
    hero = (
      <div className="hero">
        <div className="amount">{stroopsToXlm(BigInt(account.balance))} <small>XLM</small></div>
        <div className="sub">
          <Badge ok={included}>{included ? 'inclusion verified' : 'inclusion FAILED'}</Badge>
          <Tooltip text="Your balance's Merkle path was checked in-browser against the rollup's committed state root — the sequencer can't fake it." />
          {pending.total > 0 && <span className="pill">{pending.total} settling</span>}
        </div>
      </div>
    );
  }

  const recent = (history?.entries ?? []).slice(0, 4);

  return (
    <>
      <div className="panel">{hero}</div>
      {actions}
      <div className="panel">
        <div className="row">
          <h2 style={{ margin: 0 }}>Recent activity</h2>
          {recent.length > 0 && <a className="btn-inline" onClick={() => navigate('/activity')}>View all</a>}
        </div>
        {recent.length === 0 && <p className="muted" style={{ marginTop: '0.75rem' }}>No activity yet.</p>}
        {recent.map((e) => (
          <div className="list-row" key={`${e.status}-${e.id}`}>
            <div className="who">
              <span className="kind">{KIND_LABEL[e.kind] ?? e.kind}</span>
              {e.counterparty && <span className="cp">{shortHex(e.counterparty, 5)}</span>}
            </div>
            <div className="row" style={{ gap: '0.6rem' }}>
              <span className={e.kind === 'transfer_in' || e.kind === 'deposit' ? 'amt-in' : 'amt-out'}>
                {e.kind === 'transfer_in' || e.kind === 'deposit' ? '+' : '−'}
                {stroopsToXlm(BigInt(e.amount))}
              </span>
              <StatusBadge status={e.status} />
            </div>
          </div>
        ))}
      </div>
    </>
  );
}
