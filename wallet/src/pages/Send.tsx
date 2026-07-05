import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { useAccount } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { signAndSubmit } from '../api/sign';
import { classifyRecipient, stroopsToXlm, xlmToStroops } from '../format';
import { CopyableHex, ErrorText } from '../components/common';
import { Onboarding } from './Onboarding';

export function Send() {
  const { wallet } = useKey();
  const navigate = useNavigate();
  const { data: account } = useAccount(wallet?.pkX);
  const qc = useQueryClient();
  const [to, setTo] = useState('');
  const [amount, setAmount] = useState('');
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [ok, setOk] = useState<string | null>(null);

  if (!wallet) return <Onboarding />;

  const kind = classifyRecipient(to);
  const isWithdraw = kind === 'stellar';
  const available = account ? BigInt(account.balance) - BigInt(account.pending_out) : 0n;

  const validRecipient = kind === 'l2' || kind === 'stellar';
  let amountStroops: bigint | null = null;
  try {
    amountStroops = amount ? xlmToStroops(amount) : null;
  } catch {
    amountStroops = null;
  }
  const overBalance = amountStroops != null && amountStroops > available;
  const canProceed = !!account && validRecipient && amountStroops != null && !overBalance && !busy;

  async function submit() {
    if (!wallet || !account || amountStroops == null) return;
    setBusy(true);
    setError(null);
    setOk(null);
    try {
      const res = await signAndSubmit({
        sk: wallet.sk,
        to: to.trim(),
        amount: amountStroops,
        nonce: BigInt(account.pending_nonce),
        isWithdraw,
      });
      setOk(
        isWithdraw
          ? `Withdrawal queued (#${res.id}). Funds arrive at the Stellar address when the batch settles.`
          : `Sent (#${res.id}). Settles in the next batch.`,
      );
      setTo('');
      setAmount('');
      setConfirming(false);
      qc.invalidateQueries({ queryKey: ['account'] });
      qc.invalidateQueries({ queryKey: ['history'] });
    } catch (e) {
      setError(e);
      setConfirming(false);
    } finally {
      setBusy(false);
    }
  }

  // Withdrawal confirmation step (irreversible; exits the rollup to L1).
  if (confirming && isWithdraw && amountStroops != null) {
    return (
      <div className="panel">
        <a className="back" onClick={() => setConfirming(false)}>← back</a>
        <h2>Confirm withdrawal</h2>
        <p>
          Withdraw <strong>{stroopsToXlm(amountStroops)} XLM</strong> to the Stellar address{' '}
          <CopyableHex value={to.trim()} chars={8} />.
        </p>
        <p className="muted">
          This leaves the rollup and settles on Stellar L1. It can't be undone.
        </p>
        <button className="primary" onClick={submit} disabled={busy}>
          {busy ? 'Signing…' : 'Confirm withdrawal'}
        </button>
      </div>
    );
  }

  return (
    <div className="panel">
      <a className="back" onClick={() => navigate('/')}>← Wallet</a>
      <h2>Send</h2>
      <label>Recipient</label>
      <input
        placeholder="Rollup account (0x…) or Stellar address (G…)"
        value={to}
        onChange={(e) => setTo(e.target.value)}
      />
      {kind === 'l2' && <span className="pill l2">→ Rollup account</span>}
      {kind === 'stellar' && <span className="pill stellar">→ Stellar address · leaves the rollup</span>}
      {kind === 'invalid' && <p className="error">Not a rollup account or Stellar address.</p>}

      <div className="field-row">
        <label>Amount (XLM)</label>
        {account && (
          <button
            className="btn-inline"
            onClick={() => setAmount(stroopsToXlm(available))}
            disabled={available <= 0n}
          >
            Max {stroopsToXlm(available)}
          </button>
        )}
      </div>
      <input placeholder="0.0" value={amount} onChange={(e) => setAmount(e.target.value)} />
      {overBalance && <p className="error">Amount exceeds your available balance.</p>}

      <div style={{ marginTop: '1rem' }}>
        {isWithdraw ? (
          <button className="primary" onClick={() => setConfirming(true)} disabled={!canProceed}>
            Review withdrawal
          </button>
        ) : (
          <button className="primary" onClick={submit} disabled={!canProceed}>
            {busy ? 'Signing…' : 'Send'}
          </button>
        )}
      </div>
      {ok && <p className="ok">{ok}</p>}
      <ErrorText error={error} />
    </div>
  );
}
