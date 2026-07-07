// The interactive explainer: how Soribium works, for developers who know
// blockchains but not ZK. Every widget computes real hashes/signatures with
// the same crypto modules the wallet signs with.
import { useStatus } from '../api/queries';
import { CopyableHex } from '../components/common';
import { ComponentMap } from './widgets/ComponentMap';
import { MerkleExplorer } from './widgets/MerkleExplorer';
import { JourneySim } from './widgets/JourneySim';
import { ConstraintSandbox } from './widgets/ConstraintSandbox';
import { PublicInputs } from './widgets/PublicInputs';
import { DaVerifier } from './widgets/DaVerifier';

function Section({ n, title, children }: { n: string; title: string; children: React.ReactNode }) {
  return (
    <section className="learn-section">
      <h2>
        <span className="learn-n">{n}</span> {title}
      </h2>
      {children}
    </section>
  );
}

export default function Learn() {
  const { data: status } = useStatus();

  return (
    <div className="learn">
      <div className="panel hero">
        <div className="eyebrow">How Soribium works</div>
        <div className="amount learn-hero-title">
          A payments rollup where your browser checks the math.
        </div>
        <p className="muted" style={{ marginTop: '0.8rem' }}>
          A Soroban contract on Stellar holds the money. A sequencer processes payments off-chain.
          Every state change ships with an UltraHonk zero-knowledge proof the chain verifies —
          so the operator is trusted for <em>liveness</em>, never for <em>correctness</em>. This
          page is interactive, and every hash on it is computed for real, in your browser, with
          the same code the wallet uses.
        </p>
        {status && (
          <div className="kv" style={{ marginTop: '0.6rem' }}>
            <span className="k">Live right now</span><span className="dots" />
            <span className="v">batch #{status.batch_num} · root <CopyableHex value={status.root} chars={6} /></span>
          </div>
        )}
      </div>

      <Section n="01" title="Three machines, one trust boundary">
        <p>
          The wallet holds your key and signs payments. The sequencer holds the full account
          state and does all the work. The contract holds all the money — and it never sees a
          single payment. It sees exactly one thing: <strong>proofs that the state was updated
          according to the rules</strong>. Click each component.
        </p>
        <ComponentMap />
      </Section>

      <Section n="02" title="The entire bank is one number">
        <p>
          Every account is a leaf <span className="mono">{'{pubkey, balance, nonce}'}</span> in a
          Merkle tree of Poseidon2 hashes (a hash designed to be cheap <em>inside</em> ZK
          circuits). The root — 32 bytes — commits to every balance simultaneously. Change any
          account and the root changes; nothing can be edited without the whole world noticing.
          This demo tree is depth 4 (16 accounts) for legibility; production runs depth 8.
        </p>
        <MerkleExplorer />
      </Section>

      <Section n="03" title="A payment's journey">
        <p>
          This simulator runs the same state machine as the production sequencer —{' '}
          <span className="mono">building → proving → submitting → confirmed</span> — with real
          Schnorr signatures and real timing from the live deployment. Batches build eagerly
          (more than one pending payment) or after a 5-second timer, mirroring Stellar's ledger
          cadence. Try the failure buttons too.
        </p>
        <JourneySim />
      </Section>

      <Section n="04" title="What the proof actually proves">
        <p>
          A ZK circuit isn't a program that runs — it's a system of equations with blanks. The
          prover fills in the blanks (payments, signatures, Merkle paths); the proof convinces
          anyone that <em>every equation holds</em>, without revealing the values. Cheating means
          finding values that satisfy an unsatisfiable equation. These attacks run the real
          crypto — watch which rule catches each one.
        </p>
        <ConstraintSandbox />
      </Section>

      <Section n="05" title="Five numbers on-chain">
        <p>
          When a batch lands, the contract doesn't take the sequencer's word for anything it can
          check itself. It assembles the proof's public statement from its <em>own</em> trusted
          state — then runs the UltraHonk verifier natively (Stellar's Protocol 25/26 added the
          BN254 host functions that make this possible).
        </p>
        <PublicInputs />
      </Section>

      <Section n="06" title="The validium bargain">
        <p>
          Transaction data lives off-chain (this is a <em>validium</em>): the chain holds only
          the root and a commitment to the batch's transactions. Validity is trustless — no one
          can steal funds. Availability is the operator's promise: if the sequencer withheld the
          data, balances would be <em>frozen</em>, not stolen (you need your Merkle path to exit).
          The commitment makes that promise checkable — right now, by you:
        </p>
        <DaVerifier />
        <p className="muted" style={{ fontSize: '0.8rem', marginTop: '0.8rem' }}>
          Going deeper: the{' '}
          <a href="https://github.com/tomerweller/soribium" target="_blank" rel="noreferrer">source</a>,{' '}
          <a href="https://github.com/tomerweller/soribium/blob/main/DESIGN.md" target="_blank" rel="noreferrer">DESIGN.md</a>{' '}
          (hash domains, envelope, trust model), and{' '}
          <a href="https://github.com/tomerweller/soribium/blob/main/docs/PROVING.md" target="_blank" rel="noreferrer">PROVING.md</a>{' '}
          (measured proving benchmarks, recursion analysis). Or just open the Explorer tab — the
          roots-match check there is this whole page in one line.
        </p>
      </Section>
    </div>
  );
}
