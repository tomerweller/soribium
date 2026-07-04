import { useEffect, useRef, useState, type ReactNode } from 'react';
import QRCode from 'qrcode';

export function Badge({ ok, children }: { ok: boolean; children: ReactNode }) {
  return <span className={ok ? 'badge badge-ok' : 'badge badge-bad'}>{children}</span>;
}

export function StatusBadge({ status }: { status: string }) {
  const cls =
    status === 'batched' ? 'badge-ok' : status === 'rejected' ? 'badge-bad' : 'badge-pending';
  return <span className={`badge ${cls}`}>{status}</span>;
}

export function CopyButton({ text }: { text: string }) {
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
      {copied ? 'copied' : 'copy'}
    </button>
  );
}

export function Qr({ text }: { text: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) QRCode.toCanvas(ref.current, text, { width: 200, margin: 1 });
  }, [text]);
  return <canvas ref={ref} />;
}

export function ErrorText({ error }: { error: unknown }) {
  if (!error) return null;
  const msg = error instanceof Error ? error.message : String(error);
  return <p className="error">{msg}</p>;
}
