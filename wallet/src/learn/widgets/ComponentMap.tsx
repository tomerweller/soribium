// S1: the three machines and the messages between them. A pulse cycles
// through the four message types; clicking a component shows its spec card.
import { useEffect, useState } from 'react';

const COMPONENTS = {
  wallet: {
    title: 'Wallet (browser)',
    holds: 'Your L2 key (derived from a Freighter signature). Signs every payment locally.',
    cant: "Can't lie: it independently re-verifies its balance's Merkle path against the on-chain root.",
  },
  sequencer: {
    title: 'Sequencer (Fly.io)',
    holds: 'The full account tree + mempool in SQLite. Builds, proves, and submits batches.',
    cant: "Can't lie about state — every root advance needs a proof. Can only delay (censor) or withhold data.",
  },
  contract: {
    title: 'Soroban contract (Stellar)',
    holds: 'Custody of all deposited XLM, the 32-byte state root, the deposit queue, the verification key.',
    cant: 'Never sees payments — only proofs about them. Verifies UltraHonk natively (Protocol 25/26 BN254 host functions).',
  },
} as const;

type Key = keyof typeof COMPONENTS;

const FLOWS = [
  { from: 'wallet', to: 'contract', label: 'deposit (L1 tx via Freighter)', dir: 'right' },
  { from: 'wallet', to: 'sequencer', label: 'signed payment → mempool', dir: 'right' },
  { from: 'sequencer', to: 'contract', label: 'proof + new root (submit_batch)', dir: 'right' },
  { from: 'contract', to: 'wallet', label: 'withdrawal payout (L1 transfer)', dir: 'left' },
] as const;

const POS: Record<Key, { x: number; y: number }> = {
  wallet: { x: 90, y: 60 },
  sequencer: { x: 310, y: 60 },
  contract: { x: 530, y: 60 },
};

export function ComponentMap() {
  const [active, setActive] = useState<Key>('sequencer');
  const [flow, setFlow] = useState(0);

  useEffect(() => {
    const t = setInterval(() => setFlow((f) => (f + 1) % FLOWS.length), 2200);
    return () => clearInterval(t);
  }, []);

  const f = FLOWS[flow];
  const a = POS[f.from as Key];
  const b = POS[f.to as Key];

  return (
    <div className="learn-widget">
      <svg viewBox="0 0 620 120" className="learn-svg">
        <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke="var(--line-strong)" strokeWidth="1" />
        <circle r="4" fill="var(--lime)">
          <animate attributeName="cx" from={a.x} to={b.x} dur="2s" repeatCount="indefinite" />
          <animate attributeName="cy" from={a.y} to={b.y} dur="2s" repeatCount="indefinite" />
        </circle>
        {(Object.keys(COMPONENTS) as Key[]).map((k) => (
          <g key={k} style={{ cursor: 'pointer' }} onClick={() => setActive(k)}>
            <rect
              x={POS[k].x - 62} y={POS[k].y - 22} width="124" height="44"
              fill={active === k ? 'rgba(204,255,0,0.1)' : 'var(--ink-2)'}
              stroke={active === k ? 'var(--lime)' : 'var(--line-strong)'}
            />
            <text x={POS[k].x} y={POS[k].y + 4} textAnchor="middle" className="learn-svg-label"
              fill={active === k ? 'var(--lime)' : 'var(--bone)'}>
              {k.toUpperCase()}
            </text>
          </g>
        ))}
      </svg>
      <p className="muted center" style={{ fontSize: '0.74rem', marginTop: '-0.3rem' }}>▸ {f.label}</p>
      <div className="learn-card">
        <div className="learn-card-title">{COMPONENTS[active].title}</div>
        <p>{COMPONENTS[active].holds}</p>
        <p className="ok" style={{ fontSize: '0.8rem' }}>{COMPONENTS[active].cant}</p>
      </div>
    </div>
  );
}
