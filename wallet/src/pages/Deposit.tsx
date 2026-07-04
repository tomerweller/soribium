import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useParams } from '../api/queries';
import { useKey } from '../keys/KeyContext';
import { awaitTx, connectFreighter, deposit, friendbotUrl, isFunded } from '../api/stellar';
import { xlmToStroops } from '../format';
import { ErrorText } from '../components/common';
import { Onboarding } from './Onboarding';

type Step = 'idle' | 'connecting' | 'building' | 'signing' | 'awaiting' | 'done';

export function Deposit() {
  const { wallet } = useKey();
  const { data: params } = useParams();
  const qc = useQueryClient();
  const [amount, setAmount] = useState('');
  const [step, setStep] = useState<Step>('idle');
  const [gAddr, setGAddr] = useState<string | null>(null);
  const [needsFunding, setNeedsFunding] = useState(false);
  const [error, setError] = useState<unknown>(null);

  if (!wallet) return <Onboarding />;

  async function run() {
    if (!params || !wallet) return;
    setError(null);
    setNeedsFunding(false);
    try {
      setStep('connecting');
      const addr = await connectFreighter(params);
      setGAddr(addr);
      if (!(await isFunded(params, addr))) {
        setNeedsFunding(true);
        setStep('idle');
        return;
      }
      const stroops = xlmToStroops(amount);
      setStep('building');
      setStep('signing');
      const hash = await deposit(params, addr, wallet.pkX, stroops);
      setStep('awaiting');
      const ok = await awaitTx(params, hash);
      if (!ok) throw new Error('deposit transaction failed on-chain');
      setStep('done');
      qc.invalidateQueries({ queryKey: ['account'] });
      qc.invalidateQueries({ queryKey: ['status'] });
    } catch (e) {
      setError(e);
      setStep('idle');
    }
  }

  return (
    <div className="panel">
      <h2>Deposit</h2>
      <p className="muted">
        Move XLM from your Stellar account (via Freighter) into the rollup. It credits your L2
        account after the next batch settles.
      </p>
      <label>Amount (XLM)</label>
      <input placeholder="0.0" value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button onClick={run} disabled={step !== 'idle' || amount.length === 0}>
        {step === 'idle' ? 'Deposit with Freighter' : step}
      </button>
      {needsFunding && gAddr && (
        <p className="muted">
          Your account {gAddr.slice(0, 8)}… isn't funded on testnet.{' '}
          <a href={friendbotUrl(gAddr)} target="_blank" rel="noreferrer">
            Fund it with friendbot
          </a>
          , then retry.
        </p>
      )}
      {step === 'awaiting' && <p className="muted">Submitted — waiting for L1 confirmation…</p>}
      {step === 'done' && (
        <p className="ok">Deposited. Your L2 balance updates when the next batch settles (~30s).</p>
      )}
      <ErrorText error={error} />
    </div>
  );
}
