import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createBrowserRouter, RouterProvider } from 'react-router-dom';
import { KeyProvider } from './keys/KeyContext';
import { Layout } from './components/Layout';
import { Balance } from './pages/Balance';
import { Send } from './pages/Send';
import { Receive } from './pages/Receive';
import { Deposit } from './pages/Deposit';
import { Withdraw } from './pages/Withdraw';
import { History } from './pages/History';
import { Status } from './pages/Status';
import './styles.css';

const queryClient = new QueryClient();

const router = createBrowserRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      { index: true, element: <Balance /> },
      { path: 'send', element: <Send /> },
      { path: 'receive', element: <Receive /> },
      { path: 'deposit', element: <Deposit /> },
      { path: 'withdraw', element: <Withdraw /> },
      { path: 'history', element: <History /> },
      { path: 'status', element: <Status /> },
    ],
  },
]);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <KeyProvider>
        <RouterProvider router={router} />
      </KeyProvider>
    </QueryClientProvider>
  </StrictMode>,
);
