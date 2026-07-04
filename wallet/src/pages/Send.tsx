import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useAccount } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { signAndSubmit } from '../api/sign';
import { isCanonicalPkX, xlmToStroops } from '../format';
import { ErrorText } from '../components/common';
import { Onboarding } from './Onboarding';

export function Send() {
  const { wallet } = useKey();
  const { data: account } = useAccount(wallet?.pkX);
  const qc = useQueryClient();
  const [to, setTo] = useState('');
  const [amount, setAmount] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [ok, setOk] = useState<string | null>(null);

  if (!wallet) return <Onboarding />;

  const validTo = isCanonicalPkX(to);
  const canSend = !!account && validTo && amount.length > 0 && !busy;

  async function submit() {
    if (!wallet || !account) return;
    setBusy(true);
    setError(null);
    setOk(null);
    try {
      const stroops = xlmToStroops(amount);
      const res = await signAndSubmit({
        sk: wallet.sk,
        to,
        amount: stroops,
        nonce: BigInt(account.pending_nonce),
        isWithdraw: false,
      });
      setOk(`Queued (tx #${res.id}). It settles in the next batch (~30s).`);
      setTo('');
      setAmount('');
      qc.invalidateQueries({ queryKey: ['account'] });
      qc.invalidateQueries({ queryKey: ['history'] });
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="panel">
      <h2>Send</h2>
      <label>Recipient account (pk_x)</label>
      <input placeholder="0x… (64 hex)" value={to} onChange={(e) => setTo(e.target.value.trim())} />
      {to.length > 0 && !validTo && <p className="error">Not a valid account id.</p>}
      <label>Amount (XLM)</label>
      <input placeholder="0.0" value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button onClick={submit} disabled={!canSend}>
        {busy ? 'Signing…' : 'Send'}
      </button>
      {ok && <p className="ok">{ok}</p>}
      <ErrorText error={error} />
    </div>
  );
}
