// Typed client for the sequencer HTTP API (encodings frozen in DESIGN.md /
// the sequencer's api.rs). All field values are 0x+64 hex; amounts decimal
// strings of stroops; nonce a JSON number.
import { SEQUENCER_URL } from '../config';

export interface Params {
  contract_id: string;
  token_id: string;
  network_passphrase: string;
  rpc_url: string;
  batch: { deposits: number; txs: number };
}

export interface AccountInfo {
  pk_x: string;
  index: number;
  balance: string;
  nonce: number;
  pending_nonce: number;
  pending_out: string;
  root: string;
  batch_num: number;
  siblings: string[];
}

export interface Status {
  root: string;
  batch_num: number;
  pending_txs: number;
  pending_deposits: number;
  contract_id: string;
  inflight_batch: { batch_num: number; status: string } | null;
  chain_synced: boolean;
}

export interface HistoryEntry {
  id: number;
  batch_num: number | null;
  kind: 'deposit' | 'transfer_in' | 'transfer_out' | 'withdraw';
  counterparty: string | null;
  amount: string;
  nonce: number | null;
  status: 'pending' | 'batched' | 'rejected';
  ts: number;
}

export interface WireSig {
  r_x: string;
  r_y: string;
  s_lo: string;
  s_hi: string;
}

export interface TxRequest {
  from_pk_x: string;
  from_pk_y: string;
  to: string;
  amount: string;
  nonce: number;
  is_withdraw: boolean;
  sig: WireSig;
}

/** An error carrying the sequencer's structured `{code, message}`. */
export class ApiError extends Error {
  constructor(public code: string, message: string, public httpStatus: number) {
    super(message);
  }
}

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${SEQUENCER_URL}${path}`, init);
  const text = await res.text();
  const body = text ? JSON.parse(text) : null;
  if (!res.ok) {
    const err = body?.error;
    throw new ApiError(err?.code ?? 'HTTP_ERROR', err?.message ?? res.statusText, res.status);
  }
  return body as T;
}

export const api = {
  params: () => req<Params>('/params'),
  status: () => req<Status>('/status'),
  account: (pkX: string) => req<AccountInfo>(`/account/${pkX}`),
  history: (pkX: string) => req<{ entries: HistoryEntry[] }>(`/history/${pkX}`),
  batches: () => req<{ batches: unknown[] }>('/batches'),
  da: (n: number) => req<Record<string, unknown>>(`/da/${n}`),
  submitTx: (tx: TxRequest) =>
    req<{ id: number; status: string }>('/tx', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx),
    }),
};

/** GET /account, returning null on a 404 (unknown/unfunded account). */
export async function accountOrNull(pkX: string): Promise<AccountInfo | null> {
  try {
    return await api.account(pkX);
  } catch (e) {
    if (e instanceof ApiError && e.httpStatus === 404) return null;
    throw e;
  }
}
