import { createContext, useContext, useMemo, useState, type ReactNode } from 'react';
import * as keystore from './keystore';
import type { Wallet } from './keystore';

interface KeyCtx {
  wallet: Wallet | null;
  generate: () => void;
  importSk: (hex: string) => void;
  clear: () => void;
}

const Ctx = createContext<KeyCtx | null>(null);

export function KeyProvider({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<Wallet | null>(() => keystore.load());
  const value = useMemo<KeyCtx>(
    () => ({
      wallet,
      generate: () => setWallet(keystore.generate()),
      importSk: (hex: string) => setWallet(keystore.importSk(hex)),
      clear: () => {
        keystore.clear();
        setWallet(null);
      },
    }),
    [wallet],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useKey(): KeyCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useKey outside KeyProvider');
  return ctx;
}
