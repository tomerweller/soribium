// S2: an editable Merkle account tree computed with the REAL Poseidon2.
// Click a leaf to illuminate its path; edit a balance and watch the
// recomputation ripple to a new root. Every hash is genuine and copyable.
import { useMemo, useState } from 'react';
import { CopyableHex } from '../../components/common';
import { frToHex32 } from '../../crypto/fields';
import { stroopsToXlm, xlmToStroops } from '../../format';
import { DEMO_DEPTH, demoLeaves, demoLevels, makeAccounts } from '../demo';

const W = 620;
const LEVEL_H = 56;

export function MerkleExplorer() {
  const [accounts, setAccounts] = useState(() => makeAccounts());
  const [selected, setSelected] = useState<number | null>(0);
  const [edit, setEdit] = useState('');

  const levels = useMemo(() => demoLevels(demoLeaves(accounts)), [accounts]);
  const root = levels[levels.length - 1][0];

  // Path indices per level for the selected leaf.
  const pathIdx = useMemo(() => {
    if (selected == null) return new Set<string>();
    const s = new Set<string>();
    let idx = selected;
    for (let d = 0; d <= DEMO_DEPTH; d++) {
      s.add(`${d}:${idx}`);
      idx >>= 1;
    }
    return s;
  }, [selected]);

  const selAccount = accounts.find((a) => a.index === selected && a.onRollup);

  function applyEdit() {
    if (!selAccount || !edit) return;
    try {
      const bal = xlmToStroops(edit);
      setAccounts(accounts.map((a) => (a.index === selAccount.index ? { ...a, balance: bal } : a)));
      setEdit('');
    } catch {
      /* invalid amount — ignore */
    }
  }

  const x = (d: number, i: number) => (W / levels[d].length) * (i + 0.5);
  const y = (d: number) => LEVEL_H * (DEMO_DEPTH - d) + 22;

  return (
    <div className="learn-widget">
      <svg viewBox={`0 0 ${W} ${LEVEL_H * DEMO_DEPTH + 44}`} className="learn-svg">
        {levels.map((level, d) =>
          level.map((h, i) => {
            const onPath = pathIdx.has(`${d}:${i}`);
            const isLeaf = d === 0;
            const acct = isLeaf ? accounts.find((a) => a.index === i && a.onRollup) : undefined;
            const empty = h === 0n;
            return (
              <g key={`${d}:${i}`}>
                {d < DEMO_DEPTH && (
                  <line
                    x1={x(d, i)} y1={y(d) - 8}
                    x2={x(d + 1, i >> 1)} y2={y(d + 1) + 8}
                    stroke={onPath && pathIdx.has(`${d + 1}:${i >> 1}`) ? 'var(--lime)' : 'var(--line-strong)'}
                    strokeWidth="1"
                  />
                )}
                <rect
                  x={x(d, i) - (isLeaf ? 16 : 10)} y={y(d) - 8}
                  width={isLeaf ? 32 : 20} height={16}
                  fill={onPath ? 'rgba(204,255,0,0.18)' : empty ? 'transparent' : 'var(--ink-2)'}
                  stroke={onPath ? 'var(--lime)' : 'var(--line-strong)'}
                  strokeWidth={d === DEMO_DEPTH ? 1.5 : 1}
                  style={{ cursor: isLeaf ? 'pointer' : 'default' }}
                  onClick={() => isLeaf && setSelected(i)}
                />
                {acct && (
                  <text x={x(0, i)} y={y(0) + 24} textAnchor="middle" className="learn-svg-label" fill="var(--lime)">
                    {acct.name}
                  </text>
                )}
              </g>
            );
          }),
        )}
        <text x={x(DEMO_DEPTH, 0) + 18} y={y(DEMO_DEPTH) + 4} className="learn-svg-label" fill="var(--muted)">
          ← root
        </text>
      </svg>

      <div className="kv"><span className="k">Root</span><span className="dots" /><span className="v"><CopyableHex value={frToHex32(root)} chars={8} /></span></div>
      {selAccount ? (
        <>
          <div className="kv"><span className="k">{selAccount.name} leaf</span><span className="dots" /><span className="v"><CopyableHex value={frToHex32(levels[0][selAccount.index])} chars={8} /></span></div>
          <div className="kv"><span className="k">Balance / nonce</span><span className="dots" /><span className="v">{stroopsToXlm(selAccount.balance)} XLM · n{String(selAccount.nonce)}</span></div>
          <div className="row" style={{ marginTop: '0.5rem' }}>
            <input
              placeholder={`set ${selAccount.name}'s balance (XLM)…`}
              value={edit}
              onChange={(e) => setEdit(e.target.value)}
              style={{ maxWidth: '60%' }}
            />
            <button className="btn-inline" onClick={applyEdit}>recompute →</button>
          </div>
        </>
      ) : (
        <p className="muted" style={{ fontSize: '0.78rem' }}>Click a leaf. Empty slots hash to 0.</p>
      )}
    </div>
  );
}
