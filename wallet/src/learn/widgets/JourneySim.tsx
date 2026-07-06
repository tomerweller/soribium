// S3: the payment lifecycle, driven by an XState machine whose states mirror
// the production sequencer's (engine.rs / batcher.rs) by name. Signatures
// are real Schnorr; the root register is a real Poseidon2 root.
import { useMachine } from '@xstate/react';
import { CopyableHex } from '../../components/common';
import { frToHex32 } from '../../crypto/fields';
import { stroopsToXlm } from '../../format';
import { BATCH_SLOTS, paymentJourney, TIMINGS } from '../machines/paymentJourney';

const PIPELINE = ['idle', 'building', 'proving', 'submitting', 'confirmed'] as const;
const STAGE_NOTE: Record<string, string> = {
  idle: 'waiting for payments (eager: builds when >1 pending, or after 5s)',
  building: `drain mempool into ${BATCH_SLOTS} slots, pad the rest (${TIMINGS.building}ms)`,
  proving: `witness + bb prove — measured ${(TIMINGS.proving / 1000).toFixed(1)}s on the live VM`,
  submitting: 'submit_batch tx → Soroban verifies the proof on-chain',
  confirmed: 'root advances; balances final',
};

export function JourneySim() {
  const [state, send] = useMachine(paymentJourney);
  const { accounts, mempool, batch, root, batchNum, log } = state.context;
  const stage = String(state.value);

  return (
    <div className="learn-widget">
      <div className="learn-actors">
        {accounts.map((a) => (
          <div key={a.name} className={`learn-actor ${a.onRollup ? '' : 'ghost'}`}>
            <div className="learn-card-title">{a.name}</div>
            <div>{a.onRollup ? `${stroopsToXlm(a.balance)} XLM · n${String(a.nonce)}` : 'not on rollup'}</div>
          </div>
        ))}
      </div>

      <div className="learn-buttons">
        <button className="secondary" onClick={() => send({ type: 'SEND', from: 'alice', to: 'bob', amount: 5_000_000n })}>
          alice → bob 0.5
        </button>
        <button className="secondary" onClick={() => send({ type: 'SEND', from: 'bob', to: 'alice', amount: 3_000_000n })}>
          bob → alice 0.3
        </button>
        <button className="secondary" onClick={() => send({ type: 'SEND', from: 'alice', to: 'carol', amount: 1_000_000n })}>
          alice → carol 0.1
        </button>
        <button className="secondary" onClick={() => send({ type: 'SEND', from: 'alice', to: 'bob', amount: 99_000_000n })}>
          alice → bob 9.9
        </button>
        <button className="btn-inline" onClick={() => send({ type: 'RESET' })}>reset</button>
      </div>

      <div className="learn-pipeline">
        {PIPELINE.map((s, i) => (
          <div key={s} className={`learn-stage ${stage === s ? 'active' : ''}`}>
            <span className="dot">{i + 1}</span>
            {s.toUpperCase()}
          </div>
        ))}
      </div>
      <p className="muted" style={{ fontSize: '0.74rem' }}>▸ {STAGE_NOTE[stage]}</p>

      <div className="row" style={{ alignItems: 'flex-start', gap: '1rem' }}>
        <div style={{ flex: 1 }}>
          <div className="learn-card-title">MEMPOOL ({mempool.length})</div>
          {mempool.length === 0 && <p className="muted" style={{ fontSize: '0.74rem' }}>empty</p>}
          {mempool.map((t) => (
            <div key={t.id} className="learn-slot">
              #{t.id} {t.from}→{t.to} {stroopsToXlm(t.amount)} n{String(t.nonce)} sig {t.sigHex}
            </div>
          ))}
        </div>
        <div style={{ flex: 1 }}>
          <div className="learn-card-title">BATCH ({BATCH_SLOTS} slots)</div>
          {Array.from({ length: BATCH_SLOTS }).map((_, i) => {
            const t = batch[i];
            return (
              <div key={i} className={`learn-slot ${t ? 'filled' : 'pad'}`}>
                {t ? `#${t.id} ${t.from}→${t.to} ${stroopsToXlm(t.amount)}` : 'padding (identity update + fixed sig)'}
              </div>
            );
          })}
        </div>
      </div>

      <div className="kv" style={{ marginTop: '0.6rem' }}>
        <span className="k">State root · batch #{batchNum}</span><span className="dots" />
        <span className="v"><CopyableHex value={frToHex32(root)} chars={8} /></span>
      </div>

      <div className="learn-log">
        {log.map((l, i) => (
          <div key={i} className={l.kind === 'reject' ? 'error' : l.kind === 'ok' ? 'ok' : 'muted'}>
            {l.text}
          </div>
        ))}
      </div>
      <p className="muted" style={{ fontSize: '0.72rem' }}>
        Note: alice→bob then bob→alice in one batch exercises sequential application; alice→carol
        teaches RECIPIENT_UNKNOWN (carol never deposited); the 9.9 XLM one overdrafts.
      </p>
    </div>
  );
}
