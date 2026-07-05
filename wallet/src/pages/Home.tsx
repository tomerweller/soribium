import { useNavigate } from 'react-router-dom';
import { useAccount, useHistory, usePending } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { hexToFr } from '../crypto/fields';
import { leafValue, verifyPath } from '../crypto/merkle';
import { stroopsToXlm } from '../format';
import { CopyableHex, ErrorText, StatusBadge } from '../components/common';
import { VerifiedSeal } from '../components/VerifiedSeal';
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
        <div className="eyebrow">Balance</div>
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
        <div className="eyebrow">
          Balance{pending.total > 0 && <span className="pill" style={{ marginLeft: '0.6rem' }}>{pending.total} settling</span>}
        </div>
        <div className="amount">{stroopsToXlm(BigInt(account.balance))} <small>XLM</small></div>
        <VerifiedSeal verified={included} root={account.root} batch={account.batch_num} />
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
              {e.counterparty && <span className="cp"><CopyableHex value={e.counterparty} chars={5} /></span>}
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
