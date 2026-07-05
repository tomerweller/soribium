// Local record of L1 deposits that have confirmed on Stellar but whose L2
// credit hasn't landed yet (it lands when the sequencer batches the queue).
// The sequencer's per-account API has no "incoming deposit" signal, so the
// wallet tracks these locally to power the "settling" indicator, clearing an
// entry once the account balance reflects it.
const KEY = 'soribium.v1.pendingDeposits';

export interface PendingDeposit {
  pkX: string;
  amount: string; // stroops, decimal
  txHash: string;
  at: number;
}

export function list(pkX: string): PendingDeposit[] {
  try {
    const all = JSON.parse(localStorage.getItem(KEY) ?? '[]') as PendingDeposit[];
    return all.filter((d) => d.pkX === pkX);
  } catch {
    return [];
  }
}

function writeAll(all: PendingDeposit[]): void {
  localStorage.setItem(KEY, JSON.stringify(all));
}

function readAll(): PendingDeposit[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '[]') as PendingDeposit[];
  } catch {
    return [];
  }
}

export function add(d: PendingDeposit): void {
  writeAll([...readAll(), d]);
}

/**
 * Drop entries for this account once the on-chain balance has caught up to
 * (or past) the total we were waiting on — the credit has landed.
 */
export function reconcile(pkX: string, balance: bigint): void {
  const all = readAll();
  const mine = all.filter((d) => d.pkX === pkX);
  if (mine.length === 0) return;
  const owed = mine.reduce((acc, d) => acc + BigInt(d.amount), 0n);
  // We don't know the pre-deposit balance, so use a simple heuristic: once the
  // account exists and its balance is at least the total pending, assume the
  // credits landed and clear them. Ages out after 5 minutes regardless.
  const now = Date.now();
  const settled = balance >= owed;
  const kept = all.filter(
    (d) => d.pkX !== pkX || (!settled && now - d.at < 5 * 60_000),
  );
  if (kept.length !== all.length) writeAll(kept);
}
