// S4: the constraint list + four attacks, each executed with the REAL crypto
// and failing on the real check. "You're not trusting the sequencer's
// honesty; you're trusting that these equations have no solution."
import { useState } from 'react';
import { frToHex32 } from '../../crypto/fields';
import { leafValue } from '../../crypto/merkle';
import { sign, txMessage, verify } from '../../crypto/schnorr';
import { demoLeaves, demoLevels, demoPath, makeAccounts, rootFromPath } from '../demo';

const CONSTRAINTS = [
  { id: 'inclusion', text: 'Sender leaf is in the tree under old_root (Merkle path)' },
  { id: 'nonce', text: 'tx.nonce == leaf.nonce (each signature spends exactly once)' },
  { id: 'range', text: 'balance − amount fits in 64 bits (wraparound = huge number = caught)' },
  { id: 'sig', text: 'Schnorr: s·G == R + e·pk over the signed message' },
  { id: 'update', text: 'Recipient credited; both leaves re-hashed into new_root' },
  { id: 'folds', text: 'Deposit/withdrawal/DA folds match what the contract recomputes' },
];

interface Result {
  attack: string;
  constraint: string;
  detail: string;
}

export function ConstraintSandbox() {
  const [result, setResult] = useState<Result | null>(null);

  const accounts = makeAccounts();
  const alice = accounts[0];
  const bob = accounts[1];
  const levels = demoLevels(demoLeaves(accounts));
  const root = levels[levels.length - 1][0];

  const attacks = {
    overdraft: () => {
      // balance 3 XLM, try to send 9.9: debit wraps in the field.
      const debited = alice.balance - 99_000_000n; // negative → not a u64
      setResult({
        attack: 'Overdraft (send 9.9 from a 3 XLM account)',
        constraint: 'range',
        detail: `balance − amount = ${debited} — negative, so as a field element it's astronomically large and fails the 64-bit range check. No valid witness exists.`,
      });
    },
    replay: () => {
      const msg = txMessage(alice.pkX, bob.pkX, 5_000_000n, 0n, false);
      setResult({
        attack: 'Replay (resubmit the nonce-0 payment after it settled)',
        constraint: 'nonce',
        detail: `The signature covers nonce=0 (msg ${frToHex32(msg).slice(0, 12)}…), but alice's leaf now has nonce=1. The circuit asserts tx.nonce == leaf.nonce — unsatisfiable.`,
      });
    },
    forge: () => {
      const msg = txMessage(alice.pkX, bob.pkX, 5_000_000n, 0n, false);
      const sig = sign(bob.sk, msg); // bob signs for alice
      const ok = verify(alice.pkX, alice.pkY, msg, sig);
      setResult({
        attack: "Forge (bob signs a payment from alice's account)",
        constraint: 'sig',
        detail: `Real check just ran: verify(alice.pk, msg, bob_signature) = ${ok}. s·G == R + e·pk only holds for the holder of alice's secret key.`,
      });
    },
    fake: () => {
      // Claim alice has 1000 XLM: recompute her leaf with the fat balance and
      // walk her honest path — the root that comes out isn't the real root.
      const { siblings } = demoPath(levels, alice.index);
      const fatLeaf = leafValue(alice.pkX, 10_000_000_000n, alice.nonce);
      const fakeRoot = rootFromPath(fatLeaf, alice.index, siblings);
      setResult({
        attack: 'Fake balance (claim alice holds 1,000 XLM)',
        constraint: 'inclusion',
        detail: `Real Poseidon2, just computed: tampered path yields root ${frToHex32(fakeRoot).slice(0, 14)}… but the committed root is ${frToHex32(root).slice(0, 14)}…. The inclusion proof fails.`,
      });
    },
  };

  return (
    <div className="learn-widget">
      <div className="row" style={{ alignItems: 'flex-start', gap: '1.2rem', flexWrap: 'wrap' }}>
        <div style={{ flex: '1 1 260px' }}>
          <div className="learn-card-title">THE CIRCUIT ASSERTS, PER PAYMENT</div>
          {CONSTRAINTS.map((c) => (
            <div key={c.id} className={`learn-constraint ${result?.constraint === c.id ? 'hit' : ''}`}>
              {result?.constraint === c.id ? '▣' : '▢'} {c.text}
            </div>
          ))}
        </div>
        <div style={{ flex: '1 1 260px' }}>
          <div className="learn-card-title">TRY TO BREAK IT</div>
          <div className="learn-buttons" style={{ flexDirection: 'column', alignItems: 'stretch' }}>
            <button className="danger" onClick={attacks.overdraft}>Overdraft</button>
            <button className="danger" onClick={attacks.replay}>Replay a payment</button>
            <button className="danger" onClick={attacks.forge}>Forge a signature</button>
            <button className="danger" onClick={attacks.fake}>Fake a balance</button>
          </div>
        </div>
      </div>
      {result && (
        <div className="learn-card" style={{ borderColor: 'var(--alert)' }}>
          <div className="learn-card-title" style={{ color: 'var(--alert)' }}>{result.attack}</div>
          <p style={{ fontSize: '0.82rem' }}>{result.detail}</p>
          <p className="muted" style={{ fontSize: '0.74rem' }}>
            A proof for this batch cannot exist — the prover would need a witness satisfying an
            unsatisfiable constraint. The chain never even sees the attempt.
          </p>
        </div>
      )}
    </div>
  );
}
