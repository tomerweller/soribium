import { useKey } from '../keys/KeyContext';
import { CopyButton, Qr } from '../components/common';
import { Onboarding } from './Onboarding';

export function Receive() {
  const { wallet } = useKey();
  if (!wallet) return <Onboarding />;
  return (
    <div className="panel">
      <h2>Receive</h2>
      <p className="muted">Share your account id to receive payments.</p>
      <div className="row">
        <span className="mono">{wallet.pkX}</span>
        <CopyButton text={wallet.pkX} />
      </div>
      <div style={{ marginTop: '1rem' }}>
        <Qr text={wallet.pkX} />
      </div>
    </div>
  );
}
