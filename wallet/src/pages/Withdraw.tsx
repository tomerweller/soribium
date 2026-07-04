import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { StrKey } from '@stellar/stellar-sdk';
import { useAccount } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { signAndSubmit } from '../api/sign';
import { xlmToStroops } from '../format';
import { ErrorText } from '../components/common';
import { Onboarding } from './Onboarding';

function validStellarAddr(s: string): boolean {
  return StrKey.isValidEd25519PublicKey(s) || StrKey.isValidContract(s);
}

export function Withdraw() {
  const { wallet } = useKey();
  const { data: account } = useAccount(wallet?.pkX);
  const qc = useQueryClient();
  const [dest, setDest] = useState('');
  const [amount, setAmount] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [ok, setOk] = useState<string | null>(null);

  if (!wallet) return <Onboarding />;

  const validDest = validStellarAddr(dest);
  const canSend = !!account && validDest && amount.length > 0 && !busy;

  async function submit() {
    if (!wallet || !account) return;
    setBusy(true);
    setError(null);
    setOk(null);
    try {
      const res = await signAndSubmit({
        sk: wallet.sk,
        to: dest,
        amount: xlmToStroops(amount),
        nonce: BigInt(account.pending_nonce),
        isWithdraw: true,
      });
      setOk(`Withdrawal queued (tx #${res.id}). Funds arrive at the Stellar address when the batch settles.`);
      setDest('');
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
      <h2>Withdraw to Stellar</h2>
      <label>Destination address (G… or C…)</label>
      <input placeholder="G…" value={dest} onChange={(e) => setDest(e.target.value.trim())} />
      {dest.length > 0 && !validDest && <p className="error">Not a valid Stellar address.</p>}
      <label>Amount (XLM)</label>
      <input placeholder="0.0" value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button onClick={submit} disabled={!canSend}>
        {busy ? 'Signing…' : 'Withdraw'}
      </button>
      <p className="muted" style={{ fontSize: '0.8rem' }}>
        Up to 8 withdrawals settle per batch; a busy batch may push yours to the next one.
      </p>
      {ok && <p className="ok">{ok}</p>}
      <ErrorText error={error} />
    </div>
  );
}
