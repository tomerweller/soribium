import { useState } from 'react';
import { useKey } from '../keys/KeyContext';
import { ErrorText } from '../components/common';

export function Onboarding() {
  const { generate, importSk } = useKey();
  const [sk, setSk] = useState('');
  const [error, setError] = useState<unknown>(null);

  return (
    <div className="panel">
      <h2>Welcome to Soribium</h2>
      <p className="muted">
        Create a rollup account (an L2 key held in this browser) or import one.
      </p>
      <button onClick={() => generate()}>Generate new account</button>
      <label>Or import a secret key</label>
      <input
        placeholder="0x…"
        value={sk}
        onChange={(e) => setSk(e.target.value.trim())}
      />
      <button
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
        Import
      </button>
      <ErrorText error={error} />
      <p className="muted" style={{ fontSize: '0.8rem', marginTop: '1rem' }}>
        Prototype custody: the key is stored in localStorage. Don't hold real value.
      </p>
    </div>
  );
}
