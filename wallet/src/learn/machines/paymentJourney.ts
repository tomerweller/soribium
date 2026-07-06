// The centerpiece state machine — states named exactly as in the production
// sequencer (sequencer/src/engine.rs + batcher.rs): a payment is admitted to
// the mempool, then the eager batcher builds -> proves -> submits ->
// confirms. Timings are the MEASURED cloud numbers (docs/PROVING.md §3.5).
// All balance/nonce/root arithmetic here uses the real crypto via demo.ts.
import { assign, setup } from 'xstate';
import { sign, txMessage, verify } from '../../crypto/schnorr';
import { frToHex32 } from '../../crypto/fields';
import { DemoAccount, demoRoot, makeAccounts } from '../demo';

export const BATCH_SLOTS = 4; // like the live deployment (batch_n4)
export const TIMINGS = { building: 500, proving: 1300, submitting: 900, confirmed: 900 };

export interface JourneyTx {
  id: number;
  from: string;
  to: string;
  amount: bigint;
  nonce: bigint;
  sigHex: string; // real Schnorr signature (r_x), truncated for display
}

export interface LogEntry {
  kind: 'ok' | 'reject' | 'info';
  text: string;
}

interface Ctx {
  accounts: DemoAccount[];
  mempool: JourneyTx[];
  batch: JourneyTx[];
  root: bigint;
  batchNum: number;
  nextId: number;
  log: LogEntry[];
}

type Ev = { type: 'SEND'; from: string; to: string; amount: bigint } | { type: 'RESET' };

function pendingFor(ctx: Ctx, name: string): JourneyTx[] {
  return ctx.mempool.filter((t) => t.from === name);
}

/** Mempool admission — the same checks engine.rs::submit_tx performs. */
function admit(ctx: Ctx, ev: { from: string; to: string; amount: bigint }):
  | { ok: true; tx: JourneyTx; note: string }
  | { ok: false; note: string } {
  const from = ctx.accounts.find((a) => a.name === ev.from)!;
  const to = ctx.accounts.find((a) => a.name === ev.to)!;
  const pending = pendingFor(ctx, ev.from);
  const nonce = from.nonce + BigInt(pending.length);
  const pendingOut = pending.reduce((s, t) => s + t.amount, 0n);

  if (!from.onRollup) return { ok: false, note: `${ev.from}: ACCOUNT_UNKNOWN — never deposited` };
  if (!to.onRollup)
    return { ok: false, note: `RECIPIENT_UNKNOWN — ${ev.to} isn't in the tree yet (must deposit once first)` };
  if (ev.amount > from.balance - pendingOut)
    return { ok: false, note: `INSUFFICIENT_BALANCE — available ${(from.balance - pendingOut) / 10_000_000n} XLM` };

  // Real Schnorr signature over the real message hash.
  const msg = txMessage(from.pkX, to.pkX, ev.amount, nonce, false);
  const sig = sign(from.sk, msg);
  if (!verify(from.pkX, from.pkY, msg, sig)) return { ok: false, note: 'BAD_SIGNATURE' }; // never happens
  return {
    ok: true,
    tx: {
      id: ctx.nextId,
      from: ev.from,
      to: ev.to,
      amount: ev.amount,
      nonce,
      sigHex: frToHex32(sig.r_x).slice(0, 14) + '…',
    },
    note: `signed: msg=${frToHex32(msg).slice(0, 12)}… nonce=${nonce}`,
  };
}

/** Apply the batch to the accounts and recompute the REAL root. */
function applyBatch(ctx: Ctx): Pick<Ctx, 'accounts' | 'root'> {
  const accounts = ctx.accounts.map((a) => ({ ...a }));
  for (const tx of ctx.batch) {
    const from = accounts.find((a) => a.name === tx.from)!;
    const to = accounts.find((a) => a.name === tx.to)!;
    from.balance -= tx.amount;
    from.nonce += 1n;
    to.balance += tx.amount;
  }
  return { accounts, root: demoRoot(accounts) };
}

function initialCtx(): Ctx {
  const accounts = makeAccounts();
  return { accounts, mempool: [], batch: [], root: demoRoot(accounts), batchNum: 0, nextId: 1, log: [] };
}

export const paymentJourney = setup({
  types: { context: {} as Ctx, events: {} as Ev },
  guards: {
    eager: ({ context }) => context.mempool.length > 1, // the >1-pending trigger
    hasPending: ({ context }) => context.mempool.length > 0,
  },
  actions: {
    tryAdmit: assign(({ context, event }) => {
      if (event.type !== 'SEND') return {};
      const res = admit(context, event);
      const log = [
        ...(res.ok
          ? [
              { kind: 'ok', text: `${event.from} → ${event.to}: admitted (${res.note})` } as LogEntry,
            ]
          : [{ kind: 'reject', text: `${event.from} → ${event.to}: ${res.note}` } as LogEntry]),
        ...context.log,
      ].slice(0, 6);
      return res.ok
        ? { mempool: [...context.mempool, res.tx], nextId: context.nextId + 1, log }
        : { log };
    }),
    takeBatch: assign(({ context }) => ({
      batch: context.mempool.slice(0, BATCH_SLOTS),
      mempool: context.mempool.slice(BATCH_SLOTS),
      log: [{ kind: 'info', text: `batch built: ${Math.min(context.mempool.length, BATCH_SLOTS)} txs (+ padding to ${BATCH_SLOTS})` } as LogEntry, ...context.log].slice(0, 6),
    })),
    settle: assign(({ context }) => {
      const applied = applyBatch(context);
      return {
        ...applied,
        batchNum: context.batchNum + 1,
        batch: [],
        log: [
          { kind: 'ok', text: `batch #${context.batchNum + 1} confirmed — root advanced to ${frToHex32(applied.root).slice(0, 12)}…` } as LogEntry,
          ...context.log,
        ].slice(0, 6),
      };
    }),
    reset: assign(() => initialCtx()),
  },
}).createMachine({
  id: 'journey',
  context: initialCtx(),
  initial: 'idle',
  on: { RESET: { target: '.idle', actions: 'reset' } },
  states: {
    idle: {
      on: { SEND: { actions: 'tryAdmit' } },
      always: { guard: 'eager', target: 'building' },
      after: { 5000: { guard: 'hasPending', target: 'building' } }, // BATCH_MAX_WAIT_SECS
    },
    building: { entry: 'takeBatch', after: { [TIMINGS.building]: 'proving' } },
    proving: {
      on: { SEND: { actions: 'tryAdmit' } }, // mempool stays open while proving
      after: { [TIMINGS.proving]: 'submitting' },
    },
    submitting: {
      on: { SEND: { actions: 'tryAdmit' } },
      after: { [TIMINGS.submitting]: 'confirmed' },
    },
    confirmed: {
      entry: 'settle',
      on: { SEND: { actions: 'tryAdmit' } },
      after: { [TIMINGS.confirmed]: 'idle' },
    },
  },
});
