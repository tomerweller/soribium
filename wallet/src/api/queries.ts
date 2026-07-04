// react-query hooks over the sequencer API.
import { useQuery } from '@tanstack/react-query';
import { POLL_MS } from '../config';
import { accountOrNull, api } from './sequencer';

export function useParams() {
  return useQuery({ queryKey: ['params'], queryFn: api.params, staleTime: Infinity });
}

export function useStatus() {
  return useQuery({ queryKey: ['status'], queryFn: api.status, refetchInterval: POLL_MS });
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
