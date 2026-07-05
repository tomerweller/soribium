// react-query hooks over the sequencer API.
import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { POLL_MS } from '../config';
import * as pendingDeposits from '../keys/pendingDeposits';
import { accountOrNull, api } from './sequencer';

export function useParams() {
  return useQuery({ queryKey: ['params'], queryFn: api.params, staleTime: Infinity });
}

export function useStatus() {
  return useQuery({ queryKey: ['status'], queryFn: api.status, refetchInterval: POLL_MS, retry: false });
}

export function useAccount(pkX: string | undefined) {
  return useQuery({
    queryKey: ['account', pkX],
    queryFn: () => accountOrNull(pkX!),
    enabled: !!pkX,
    refetchInterval: POLL_MS,
  });
}

export function useHistory(pkX: string | undefined) {
  return useQuery({
    queryKey: ['history', pkX],
    queryFn: () => api.history(pkX!),
    enabled: !!pkX,
    refetchInterval: POLL_MS,
  });
}

export function useBatches() {
  return useQuery({ queryKey: ['batches'], queryFn: api.batches, refetchInterval: POLL_MS });
}

export interface Pending {
  sending: number; // outgoing L2 txs/withdrawals not yet batched
  depositing: number; // L1 deposits confirmed but not yet credited
  total: number;
}

/**
 * The account's in-flight items, for the global "settling" indicator:
 * outgoing history entries still `pending`, plus locally-tracked deposits
 * whose credit hasn't landed (reconciled against the live balance).
 */
export function usePending(pkX: string | undefined): Pending {
  const { data: history } = useHistory(pkX);
  const { data: account } = useAccount(pkX);

  // Clear settled deposits once the balance reflects them.
  useEffect(() => {
    if (pkX && account) pendingDeposits.reconcile(pkX, BigInt(account.balance));
  }, [pkX, account]);

  const sending = (history?.entries ?? []).filter(
    (e) => e.status === 'pending' && (e.kind === 'transfer_out' || e.kind === 'withdraw'),
  ).length;
  const depositing = pkX ? pendingDeposits.list(pkX).length : 0;
  return { sending, depositing, total: sending + depositing };
}
