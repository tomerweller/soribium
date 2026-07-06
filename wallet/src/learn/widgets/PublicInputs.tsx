// S5: what actually lands on-chain — the 5 public inputs assembled by the
// contract (folds computed here with the real Poseidon2) and the verify cost
// gauge. Cost is independent of batch size: verification is logarithmic.
import { useMemo, useState } from 'react';
import { CopyableHex } from '../../components/common';
import { frToHex32 } from '../../crypto/fields';
import { p2, DOMAIN_DEP } from '../../crypto/poseidon2';
import { daFold, txMessage } from '../../crypto/schnorr';
import { demoRoot, makeAccounts } from '../demo';

export function PublicInputs() {
  const [step, setStep] = useState(0);

  const data = useMemo(() => {
    const accounts = makeAccounts();
    const [alice, bob] = accounts;
    const oldRoot = demoRoot(accounts);
    const applied = accounts.map((a) => ({ ...a }));
    applied[0].balance -= 5_000_000n;
    applied[0].nonce += 1n;
    applied[1].balance += 5_000_000n;
    const newRoot = demoRoot(applied);
    // One example deposit fold + the DA fold of one payment — real hashes.
    const depositHash = p2([DOMAIN_DEP, 0n, alice.pkX, 10_000_000n]);
    const msg = txMessage(alice.pkX, bob.pkX, 5_000_000n, 0n, false);
    const daCommitment = daFold(0n, msg);
    return { oldRoot, newRoot, depositHash, daCommitment };
  }, []);

  const PIS = [
    { name: 'old_root', v: data.oldRoot, src: 'contract storage — the sequencer cannot choose it', trusted: true },
    { name: 'new_root', v: data.newRoot, src: 'envelope — but the proof forces it to follow from old_root', trusted: false },
    { name: 'deposit_hash', v: data.depositHash, src: 'recomputed on-chain from the contract\'s own deposit queue', trusted: true },
    { name: 'withdraw_hash', v: 0n, src: 'recomputed on-chain from the envelope\'s payout list', trusted: true },
    { name: 'da_commitment', v: data.daCommitment, src: 'envelope — binds the off-chain tx blob (§ next section)', trusted: false },
  ];

  return (
    <div className="learn-widget">
      <div className="learn-card-title">THE 160-BYTE STATEMENT (5 × 32-byte field elements)</div>
      {PIS.map((pi, i) => (
        <div key={pi.name} className={`learn-pi ${i <= step ? 'on' : ''}`}>
          <div className="row">
            <span className="mono" style={{ fontSize: '0.8rem', color: i <= step ? 'var(--lime)' : 'var(--muted)' }}>
              [{i}] {pi.name}
            </span>
            {i <= step && <CopyableHex value={frToHex32(pi.v)} chars={6} />}
          </div>
          {i <= step && <p className="muted" style={{ fontSize: '0.72rem', margin: '0.15rem 0 0' }}>{pi.src}</p>}
        </div>
      ))}
      <div className="learn-buttons">
        <button className="secondary" onClick={() => setStep((s) => Math.min(s + 1, 4))} disabled={step >= 4}>
          assemble next input
        </button>
        <button className="btn-inline" onClick={() => setStep(0)}>restart</button>
      </div>

      {step >= 4 && (
        <>
          <div className="learn-card" style={{ borderColor: 'var(--lime)' }}>
            <div className="learn-card-title" style={{ color: 'var(--lime)' }}>▣ ULTRAHONK.VERIFY(proof, statement)</div>
            <div className="learn-gauge">
              <div className="learn-gauge-fill" style={{ width: '23%' }} />
            </div>
            <p className="muted" style={{ fontSize: '0.74rem' }}>
              ~90M of 400M allowed CPU instructions · ~0.12 XLM per batch · one BN254 pairing check
              + MSMs via Protocol 25/26 host functions. Cost is (log-)independent of how many
              payments hide behind the proof.
            </p>
          </div>
          <p className="muted" style={{ fontSize: '0.76rem' }}>
            Two rules make this unfoolable: the proof binds <span className="mono">old_root</span>{' '}
            (replaying an old batch fails instantly), and the contract recomputes the folds from
            state it already trusts — the sequencer can't lie about deposits or payouts.
          </p>
        </>
      )}
    </div>
  );
}
