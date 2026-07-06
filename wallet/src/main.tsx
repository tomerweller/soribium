import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createBrowserRouter, RouterProvider } from 'react-router-dom';
import { KeyProvider } from './keys/KeyContext';
import { Layout } from './components/Layout';
import { Home } from './pages/Home';
import { Send } from './pages/Send';
import { Receive } from './pages/Receive';
import { Deposit } from './pages/Deposit';
import { Activity } from './pages/Activity';
import { Explorer } from './pages/Explorer';
import './styles.css';

// The explainer (and xstate) load in their own chunk.
const Learn = lazy(() => import('./learn/Learn'));

const queryClient = new QueryClient();

const router = createBrowserRouter(
  [
    {
      path: '/',
      element: <Layout />,
      children: [
        { index: true, element: <Home /> },
        // Actions launched from Home (not in the main nav).
        { path: 'send', element: <Send /> },
        { path: 'deposit', element: <Deposit /> },
        { path: 'receive', element: <Receive /> },
        // Primary destinations.
        { path: 'activity', element: <Activity /> },
        { path: 'explorer', element: <Explorer /> },
        {
          path: 'learn',
          element: (
            <Suspense fallback={<div className="panel muted">loading…</div>}>
              <Learn />
            </Suspense>
          ),
        },
      ],
    },
  ],
  // Subpath hosting (e.g. GitHub Pages /soribium/).
  { basename: import.meta.env.BASE_URL },
);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <KeyProvider>
        <RouterProvider router={router} />
      </KeyProvider>
    </QueryClientProvider>
  </StrictMode>,
);
