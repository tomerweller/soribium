import { useState } from 'react';
import { useKey } from '../keys/KeyContext';
import { ErrorText } from '../components/common';

export function Onboarding() {
  const { deriveFromFreighter, importSk, generate } = useKey();
  const [busy, setBusy] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [sk, setSk] = useState('');
  const [error, setError] = useState<unknown>(null);

  async function connect() {
    setBusy(true);
    setError(null);
    try {
      await deriveFromFreighter();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="panel">
      <h2>New account</h2>
      <p className="muted">
        Your rollup account is derived from your Stellar wallet. Sign one message in Freighter —
        that signature deterministically produces your L2 key. Nothing moves, and you can restore
        the account on any device by reconnecting the same wallet. No seed phrase to write down.
      </p>
      <button className="primary" onClick={connect} disabled={busy}>
        {busy ? 'Waiting for Freighter…' : 'Connect Freighter & derive account'}
      </button>
      <ErrorText error={error} />

      <p style={{ marginTop: '1.25rem' }}>
        <a className="back" onClick={() => setAdvanced((a) => !a)}>
          {advanced ? '− advanced' : '+ advanced'}
        </a>
      </p>
      {advanced && (
        <div>
          <label>Import a raw L2 secret key</label>
          <input placeholder="0x…" value={sk} onChange={(e) => setSk(e.target.value.trim())} />
          <button
            className="secondary"
            style={{ marginTop: '0.6rem' }}
            onClick={() => {
              try {
                setError(null);
                importSk(sk);
              } catch (e) {
                setError(e);
              }
            }}
            disabled={!sk}
          >
            Import key
          </button>
          <p className="muted" style={{ fontSize: '0.7rem', marginTop: '0.8rem' }}>
            Or generate a throwaway key not linked to any wallet (testing only — not recoverable):
          </p>
          <button className="secondary" onClick={() => generate()}>Generate throwaway key</button>
        </div>
      )}
    </div>
  );
}
