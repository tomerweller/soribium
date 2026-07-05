// The signature ZK motif. The Wallet hero runs the real client-side
// verifyPath() over the account's 8-deep Merkle path; this makes that proof
// visible: the path illuminates leaf → root, then locks into an
// INCLUSION VERIFIED seal. Failure turns the path red. Honors
// prefers-reduced-motion (renders the settled state instantly) and re-runs
// the animation whenever the root/batch changes (a freshly-proven state).
import { useEffect, useRef, useState } from 'react';
import { shortHex } from '../format';

const DEPTH = 8;

function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
}

export function VerifiedSeal({
  verified,
  root,
  batch,
}: {
  verified: boolean;
  root: string;
  batch: number;
}) {
  // `lit` = how many path levels have illuminated so far (0..DEPTH).
  const [lit, setLit] = useState(0);
  const timers = useRef<number[]>([]);

  useEffect(() => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
    if (prefersReducedMotion()) {
      setLit(DEPTH);
      return;
    }
    setLit(0);
    // Stagger the leaf→root sweep (~70ms/level ≈ 560ms total).
    for (let i = 1; i <= DEPTH; i++) {
      timers.current.push(window.setTimeout(() => setLit(i), i * 70));
    }
    return () => timers.current.forEach(clearTimeout);
    // Re-run whenever a new state is proven.
  }, [root, batch, verified]);

  const complete = lit >= DEPTH;
  const color = verified ? 'var(--lime)' : 'var(--alert)';

  // A compact vertical ladder: DEPTH sibling cells folding up into the root.
  const CELL = 9;
  const GAP = 4;
  const width = 150;
  const rungY = (i: number) => 6 + (DEPTH - 1 - i) * (CELL + GAP);
  const height = rungY(0) + CELL + 6;

  return (
    <div className="seal" aria-label={verified ? 'inclusion verified' : 'inclusion failed'}>
      <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="seal-svg">
        {/* spine */}
        <line x1="16" y1={rungY(DEPTH - 1) + CELL / 2} x2="16" y2={rungY(0) + CELL / 2}
          stroke="var(--line-strong)" strokeWidth="1" />
        {Array.from({ length: DEPTH }).map((_, i) => {
          const on = lit > i;
          const y = rungY(i);
          return (
            <g key={i}>
              <line x1="16" y1={y + CELL / 2} x2="34" y2={y + CELL / 2}
                stroke={on ? color : 'var(--line-strong)'} strokeWidth="1" />
              <rect x="10" y={y} width={CELL} height={CELL}
                fill={on ? color : 'transparent'} stroke={on ? color : 'var(--line-strong)'} strokeWidth="1" />
              <rect x="34" y={y} width={CELL} height={CELL}
                fill="none" stroke={on ? color : 'var(--line)'} strokeWidth="1" opacity={on ? 0.7 : 0.4} />
            </g>
          );
        })}
        {/* root node */}
        <rect x="9" y={rungY(DEPTH - 1) - 4} width="14" height="14"
          fill={complete ? color : 'transparent'} stroke={complete ? color : 'var(--line-strong)'} strokeWidth="1.5" />
      </svg>
      <div className="seal-readout">
        <div className={`seal-status ${verified ? 'ok' : 'bad'}`} style={{ color }}>
          <span className="seal-glyph">{complete ? (verified ? '▣' : '▨') : '▢'}</span>
          {verified ? 'INCLUSION VERIFIED' : 'INCLUSION FAILED'}
        </div>
        <div className="seal-meta">
          root {shortHex(root, 6)} · batch #{batch}
        </div>
      </div>
    </div>
  );
}
