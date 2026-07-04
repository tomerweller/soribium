import { NavLink, Outlet } from 'react-router-dom';
import { useKey } from '../keys/KeyContext';
import { shortHex } from '../format';

const tabs = [
  ['/', 'Balance'],
  ['/send', 'Send'],
  ['/receive', 'Receive'],
  ['/deposit', 'Deposit'],
  ['/withdraw', 'Withdraw'],
  ['/history', 'History'],
  ['/status', 'Status'],
];

export function Layout() {
  const { wallet } = useKey();
  return (
    <div className="app">
      <header>
        <h1>Soribium</h1>
        {wallet && <span className="account">{shortHex(wallet.pkX, 8)}</span>}
      </header>
      <nav>
        {tabs.map(([to, label]) => (
          <NavLink key={to} to={to} end={to === '/'}>
            {label}
          </NavLink>
        ))}
      </nav>
      <main>
        <Outlet />
      </main>
    </div>
  );
}
