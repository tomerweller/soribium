// Fixture-backed mock of the sequencer API for backendless wallet dev.
// Maintains a REAL Poseidon2 depth-8 tree (reusing @zkpassport/poseidon2) so
// the Balance page's client-side inclusion badge is truthfully green. Any
// queried account is auto-seeded with a starting balance for zero-friction
// dev. Not a validator — it does not verify signatures or prove anything.
import { createServer } from 'node:http';
import { poseidon2Hash } from '@zkpassport/poseidon2';

const PORT = 8787;
const DEPTH = 8;
const DOMAIN_LEAF = 1n;
const SEED_BALANCE = 5000000n; // 0.5 XLM

const p2 = (xs) => poseidon2Hash(xs);
const toHex = (v) => '0x' + v.toString(16).padStart(64, '0');
const fromHex = (s) => BigInt(s);

// idx -> {pkX(bigint), balance(bigint), nonce(bigint)}
const leaves = new Map();
let batchNum = 0;
const history = new Map(); // pkXHex -> entries[]

function leafValue(a) {
  return a ? p2([DOMAIN_LEAF, a.pkX, a.balance, a.nonce]) : 0n;
}
function level0() {
  const out = [];
  for (let i = 0; i < 1 << DEPTH; i++) out.push(leafValue(leaves.get(i)));
  return out;
}
function root() {
  let lvl = level0();
  while (lvl.length > 1) {
    const next = [];
    for (let i = 0; i < lvl.length; i += 2) next.push(p2([lvl[i], lvl[i + 1]]));
    lvl = next;
  }
  return lvl[0];
}
function path(index) {
  let lvl = level0();
  const siblings = [];
  let idx = index;
  for (let i = 0; i < DEPTH; i++) {
    siblings.push(lvl[idx ^ 1]);
    const next = [];
    for (let j = 0; j < lvl.length; j += 2) next.push(p2([lvl[j], lvl[j + 1]]));
    lvl = next;
    idx >>= 1;
  }
  return siblings;
}
function findOrSeed(pkXHex) {
  const pkX = fromHex(pkXHex);
  for (const [idx, a] of leaves) if (a.pkX === pkX) return idx;
  const idx = leaves.size;
  leaves.set(idx, { pkX, balance: SEED_BALANCE, nonce: 0n });
  return idx;
}

function json(res, code, body) {
  res.writeHead(code, { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' });
  res.end(JSON.stringify(body));
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const parts = url.pathname.split('/').filter(Boolean);

  if (parts[0] === 'healthz') return json(res, 200, 'ok');
  if (parts[0] === 'params')
    return json(res, 200, {
      contract_id: 'CMOCK000000000000000000000000000000000000000000000000000',
      token_id: 'CMOCKTOKEN00000000000000000000000000000000000000000000000',
      network_passphrase: 'Test SDF Network ; September 2015',
      rpc_url: 'https://soroban-testnet.stellar.org',
      batch: { deposits: 4, txs: 16 },
      domains: { leaf: 1, tx: 2, sig: 3, dep: 4, wd: 5, addr: 6, da: 7 },
    });
  if (parts[0] === 'status')
    return json(res, 200, {
      root: toHex(root()), batch_num: batchNum, pending_txs: 0, pending_deposits: 0,
      contract_id: 'CMOCK000000000000000000000000000000000000000000000000000',
      inflight_batch: null, chain_synced: true,
    });
  if (parts[0] === 'account') {
    const pkXHex = parts[1];
    const idx = findOrSeed(pkXHex);
    const a = leaves.get(idx);
    return json(res, 200, {
      pk_x: pkXHex, index: idx, balance: a.balance.toString(), nonce: Number(a.nonce),
      pending_nonce: Number(a.nonce), pending_out: '0',
      root: toHex(root()), batch_num: batchNum, siblings: path(idx).map(toHex),
    });
  }
  if (parts[0] === 'history')
    return json(res, 200, { entries: history.get(parts[1]) ?? [] });
  if (parts[0] === 'batches') return json(res, 200, { batches: [] });

  if (parts[0] === 'tx' && req.method === 'POST') {
    let body = '';
    for await (const chunk of req) body += chunk;
    const tx = JSON.parse(body);
    // Mock "settles" immediately: apply to the tree and record history.
    const fromIdx = findOrSeed(tx.from_pk_x);
    const from = leaves.get(fromIdx);
    const amount = BigInt(tx.amount);
    if (Number(from.nonce) !== tx.nonce)
      return json(res, 409, { error: { code: 'NONCE_MISMATCH', message: `expected ${from.nonce}` } });
    if (from.balance < amount)
      return json(res, 409, { error: { code: 'INSUFFICIENT_BALANCE', message: 'too low' } });
    from.balance -= amount;
    from.nonce += 1n;
    if (!tx.is_withdraw) {
      const toIdx = findOrSeed(tx.to);
      leaves.get(toIdx).balance += amount;
    }
    batchNum += 1;
    const push = (pk, e) => history.set(pk, [{ id: batchNum * 10 + fromIdx, batch_num: batchNum, status: 'batched', ts: Math.floor(Date.now() / 1000), ...e }, ...(history.get(pk) ?? [])]);
    if (tx.is_withdraw) push(tx.from_pk_x, { kind: 'withdraw', counterparty: tx.to, amount: tx.amount, nonce: tx.nonce });
    else { push(tx.from_pk_x, { kind: 'transfer_out', counterparty: tx.to, amount: tx.amount, nonce: tx.nonce }); push(tx.to, { kind: 'transfer_in', counterparty: tx.from_pk_x, amount: tx.amount, nonce: null }); }
    return json(res, 200, { id: batchNum, status: 'pending' });
  }

  json(res, 404, { error: { code: 'NOT_FOUND', message: url.pathname } });
});

server.listen(PORT, () => console.log(`mock sequencer on http://localhost:${PORT}`));
