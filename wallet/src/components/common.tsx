import { useEffect, useRef, useState, type ReactNode } from 'react';
import QRCode from 'qrcode';
import { friendlyError } from '../errors';
import { ApiError } from '../api/sequencer';
import { shortHex } from '../format';

export function Badge({ ok, children }: { ok: boolean; children: ReactNode }) {
  return <span className={ok ? 'badge badge-ok' : 'badge badge-bad'}>{children}</span>;
}

export function StatusBadge({ status }: { status: string }) {
  const cls =
    status === 'batched' ? 'badge-ok' : status === 'rejected' ? 'badge-bad' : 'badge-pending';
  return <span className={`badge ${cls}`}>{status}</span>;
}

export function CopyButton({ text, label = 'copy' }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className="btn-inline"
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      {copied ? 'copied ✓' : label}
    </button>
  );
}

/**
 * A shortened hex/strkey value rendered as a click-to-copy chip. The whole
 * thing copies the FULL value; hover shows it via the title attribute.
 * Use everywhere an address, root, or hash is truncated for display.
 */
export function CopyableHex({ value, chars = 6 }: { value: string; chars?: number }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className="copyable mono"
      title={copied ? 'copied' : value}
      onClick={async (e) => {
        e.stopPropagation();
        await navigator.clipboard.writeText(value);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      {shortHex(value, chars)}
      <span className="copy-ico">{copied ? '✓' : '⧉'}</span>
    </button>
  );
}

export function Qr({ text, size = 200 }: { text: string; size?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) QRCode.toCanvas(ref.current, text, { width: size, margin: 1 });
  }, [text, size]);
  return <canvas ref={ref} className="qr" />;
}

export function ErrorText({ error }: { error: unknown }) {
  if (!error) return null;
  // RECIPIENT_UNKNOWN isn't a user mistake — it's expected for a brand-new
  // payee — so present it as a neutral hint rather than a red error.
  const isHint = error instanceof ApiError && error.code === 'RECIPIENT_UNKNOWN';
  return <p className={isHint ? 'hint' : 'error'}>{friendlyError(error)}</p>;
}

/** Inline hover/focus tooltip via a title-styled superscript marker. */
export function Tooltip({ text }: { text: string }) {
  return (
    <span className="tip" tabIndex={0} aria-label={text}>
      ?<span className="tip-bubble">{text}</span>
    </span>
  );
}

/** Horizontal step indicator; `current` is the 0-based active index, or the
 * step count when complete. */
export function Stepper({ steps, current }: { steps: string[]; current: number }) {
  return (
    <ol className="stepper">
      {steps.map((s, i) => (
        <li key={s} className={i < current ? 'done' : i === current ? 'active' : ''}>
          <span className="dot">{i < current ? '✓' : i + 1}</span>
          <span className="step-label">{s}</span>
        </li>
      ))}
    </ol>
  );
}

export function Banner({ tone, children }: { tone: 'info' | 'warn'; children: ReactNode }) {
  return <div className={`banner banner-${tone}`}>{children}</div>;
}

/** Lightweight dropdown anchored to a trigger; closes on outside click / Esc. */
export function Dropdown({ trigger, children }: { trigger: ReactNode; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onEsc = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onEsc);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onEsc);
    };
  }, [open]);
  return (
    <div className="dropdown" ref={ref}>
      <button className="chip" onClick={() => setOpen((o) => !o)}>
        {trigger} <span className="caret">▾</span>
      </button>
      {open && <div className="dropdown-menu" onClick={() => setOpen(false)}>{children}</div>}
    </div>
  );
}
