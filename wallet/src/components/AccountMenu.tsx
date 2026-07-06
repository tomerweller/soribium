import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useKey } from '../keys/KeyContext';
import * as keystore from '../keys/keystore';
import { shortHex } from '../format';
import { CopyButton, Dropdown } from './common';

export function AccountMenu() {
  const { wallet, clear } = useKey();
  const navigate = useNavigate();
  const [revealed, setRevealed] = useState(false);
  if (!wallet) return null;

  const sk = keystore.exportSk();

  return (
    <Dropdown trigger={<span className="mono">{shortHex(wallet.pkX, 6)}</span>}>
      <div className="row" style={{ padding: '0.4rem 0.6rem' }}>
        <span className="mono" style={{ fontSize: '0.78rem' }}>{shortHex(wallet.pkX, 8)}</span>
        <CopyButton text={wallet.pkX} label="copy address" />
      </div>
      <a onClick={() => navigate('/receive')}>Show QR / receive</a>
      <div className="sep" />
      {wallet.linkedAddress ? (
        <div style={{ padding: '0.4rem 0.6rem' }}>
          <div className="muted" style={{ fontSize: '0.68rem', textTransform: 'uppercase', letterSpacing: '0.1em' }}>
            Linked to Stellar
          </div>
          <div className="mono" style={{ fontSize: '0.74rem', marginTop: '0.2rem' }}>
            {shortHex(wallet.linkedAddress, 6)}
          </div>
          <p className="muted" style={{ fontSize: '0.7rem', margin: '0.4rem 0 0' }}>
            Restore this account on any device by reconnecting this wallet — no key to back up.
          </p>
        </div>
      ) : !revealed ? (
        <button onClick={() => setRevealed(true)}>Back up secret key</button>
      ) : (
        <div style={{ padding: '0.4rem 0.6rem' }}>
          <p className="error" style={{ margin: '0 0 0.4rem', fontSize: '0.78rem' }}>
            Anyone with this key controls the account. Never share it.
          </p>
          <div className="mono" style={{ fontSize: '0.72rem' }}>{sk}</div>
          {sk && <CopyButton text={sk} label="copy secret key" />}
        </div>
      )}
      <div className="sep" />
      <button
        className="danger"
        onClick={() => {
          if (
            confirm(
              'Switch to a new account? Back up your current secret key first — this device will forget it.',
            )
          ) {
            clear();
            navigate('/');
          }
        }}
      >
        Switch / new account
      </button>
    </Dropdown>
  );
}
