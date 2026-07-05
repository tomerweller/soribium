import { useNavigate } from 'react-router-dom';
import { useKey } from '../keys/KeyContext';
import { CopyButton, Qr } from '../components/common';
import { Onboarding } from './Onboarding';

export function Receive() {
  const { wallet } = useKey();
  const navigate = useNavigate();
  if (!wallet) return <Onboarding />;
  return (
    <div className="panel center">
      <a className="back" onClick={() => navigate('/')} style={{ float: 'left' }}>← Home</a>
      <h2>Receive</h2>
      <p className="muted">Share your account id to get paid on the rollup.</p>
      <div style={{ display: 'flex', justifyContent: 'center', margin: '1rem 0' }}>
        <Qr text={wallet.pkX} />
      </div>
      <div className="mono">{wallet.pkX}</div>
      <div style={{ marginTop: '0.5rem' }}>
        <CopyButton text={wallet.pkX} label="copy address" />
      </div>
    </div>
  );
}
