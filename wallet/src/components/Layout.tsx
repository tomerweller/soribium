import { NavLink, Outlet } from 'react-router-dom';
import { useKey } from '../keys/KeyContext';
import { usePending, useStatus } from '../api/queries';
import { AccountMenu } from './AccountMenu';
import { Banner } from './common';

const tabs = [
  ['/', 'Wallet'],
  ['/activity', 'Activity'],
  ['/explorer', 'Explorer'],
  ['/learn', 'Learn'],
];

export function Layout() {
  const { wallet } = useKey();
  const { data: status, isError } = useStatus();
  const pending = usePending(wallet?.pkX);

  return (
    <div className="app">
      <header>
        <span className="wordmark">Soribium<span className="cursor" /></span>
        {wallet && <AccountMenu />}
      </header>
      <nav>
        {tabs.map(([to, label]) => (
          <NavLink key={to} to={to} end={to === '/'}>
            {label}
          </NavLink>
        ))}
      </nav>
      <main>
        {isError && <Banner tone="warn">Can't reach the sequencer — showing last-known state.</Banner>}
        {status && !status.chain_synced && (
          <Banner tone="warn">Sequencer is out of sync with the chain; batching is paused.</Banner>
        )}
        {pending.total > 0 && (
          <Banner tone="info">
            {pending.total} transaction{pending.total > 1 ? 's' : ''} settling
            {status?.inflight_batch ? ` — batch #${status.inflight_batch.batch_num} building…` : ' — next batch…'}
          </Banner>
        )}
        <Outlet />
      </main>
    </div>
  );
}
