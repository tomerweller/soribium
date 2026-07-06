// L1 (Stellar testnet) glue: the Freighter deposit flow and the on-chain
// root() read for the explorer page. Verified against @stellar/stellar-sdk
// 16.x and @stellar/freighter-api 6.x.
import {
  Account,
  Address,
  BASE_FEE,
  Contract,
  nativeToScVal,
  rpc,
  scValToNative,
  TransactionBuilder,
} from '@stellar/stellar-sdk';
import freighter from '@stellar/freighter-api';
import { FRIENDBOT } from '../config';
import { pkxToBytes32 } from '../format';
import { KEY_DERIVATION_MESSAGE } from '../crypto/derive';
import type { Params } from './sequencer';

/** Normalize Freighter's signMessage result to raw signature bytes. v6
 *  returns a Buffer (v3 extension) or a base64 string (v4). */
function toSigBytes(m: Buffer | string | null): Uint8Array {
  if (!m) throw new Error('No signature returned. The request may have been rejected.');
  if (typeof m !== 'string') return new Uint8Array(m);
  try {
    return Uint8Array.from(atob(m), (c) => c.charCodeAt(0));
  } catch {
    const hex = m.startsWith('0x') ? m.slice(2) : m;
    const bytes = hex.match(/.{2}/g);
    if (!bytes) throw new Error('Unrecognized signature encoding');
    return Uint8Array.from(bytes.map((h) => parseInt(h, 16)));
  }
}

/**
 * Connect Freighter and sign the fixed key-derivation message. Returns the
 * Stellar address (the L1 owner we bind to) and the raw signature bytes the
 * L2 key is derived from. No network passphrase is passed, so the signed
 * content is exactly the message — the derivation is network-independent.
 */
export async function deriveKeyMaterial(): Promise<{ address: string; sig: Uint8Array }> {
  const connected = await freighter.isConnected();
  if (connected.error || !connected.isConnected) {
    throw new Error('Freighter is not installed or unavailable');
  }
  const access = await freighter.requestAccess();
  if (access.error) throw new Error(access.error);
  const res = await freighter.signMessage(KEY_DERIVATION_MESSAGE, { address: access.address });
  if (res.error) throw new Error(String(res.error));
  return { address: access.address, sig: toSigBytes(res.signedMessage) };
}

function server(params: Params): rpc.Server {
  return new rpc.Server(params.rpc_url);
}

/** Connect Freighter and hard-check its network matches the sequencer's. */
export async function connectFreighter(params: Params): Promise<string> {
  const connected = await freighter.isConnected();
  if (connected.error || !connected.isConnected) {
    throw new Error('Freighter is not installed or unavailable');
  }
  const access = await freighter.requestAccess();
  if (access.error) throw new Error(access.error);
  const net = await freighter.getNetwork();
  if (net.error) throw new Error(net.error);
  if (net.networkPassphrase !== params.network_passphrase) {
    throw new Error(
      `Freighter is on the wrong network (${net.network}); switch it to the one matching ${params.network_passphrase}`,
    );
  }
  return access.address;
}

/** True if the account exists/funded; false → show a friendbot link. */
export async function isFunded(params: Params, address: string): Promise<boolean> {
  try {
    await server(params).getAccount(address);
    return true;
  } catch {
    return false;
  }
}

export function friendbotUrl(address: string): string {
  return `${FRIENDBOT}?addr=${encodeURIComponent(address)}`;
}

/**
 * Build, Freighter-sign, and submit a `deposit(from, l2_pk_x, amount)` call.
 * Returns the tx hash; poll status separately.
 */
export async function deposit(
  params: Params,
  from: string,
  l2PkXHex: string,
  amountStroops: bigint,
): Promise<string> {
  const srv = server(params);
  const account = await srv.getAccount(from);
  const contract = new Contract(params.contract_id);
  const op = contract.call(
    'deposit',
    Address.fromString(from).toScVal(),
    nativeToScVal(pkxToBytes32(l2PkXHex), { type: 'bytes' }),
    nativeToScVal(amountStroops, { type: 'i128' }),
  );
  const built = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: params.network_passphrase,
  })
    .addOperation(op)
    .setTimeout(60)
    .build();

  const prepared = await srv.prepareTransaction(built);
  const signed = await freighter.signTransaction(prepared.toXDR(), {
    networkPassphrase: params.network_passphrase,
    address: from,
  });
  if (signed.error) throw new Error(signed.error);
  const tx = TransactionBuilder.fromXDR(signed.signedTxXdr, params.network_passphrase);
  const sent = await srv.sendTransaction(tx);
  if (sent.status === 'ERROR') {
    throw new Error(`deposit submission failed: ${JSON.stringify(sent.errorResult)}`);
  }
  return sent.hash;
}

/** Poll a submitted tx to a terminal state. */
export async function awaitTx(params: Params, hash: string): Promise<boolean> {
  const srv = server(params);
  for (let i = 0; i < 30; i++) {
    const r = await srv.getTransaction(hash);
    if (r.status === rpc.Api.GetTransactionStatus.SUCCESS) return true;
    if (r.status === rpc.Api.GetTransactionStatus.FAILED) return false;
    await new Promise((res) => setTimeout(res, 2000));
  }
  return false;
}

/**
 * Read the contract's committed root via simulate of the `root()` view.
 * Returns 0x+64 hex, or null if the simulate is unavailable.
 */
export async function readOnchainRoot(params: Params): Promise<string | null> {
  try {
    const srv = server(params);
    const contract = new Contract(params.contract_id);
    // A read-only simulate needs a source account, but it need not exist or be
    // funded — the all-zero account id is fine since nothing is submitted.
    const dummy = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
    const built = new TransactionBuilder(new Account(dummy, '0'), {
      fee: BASE_FEE,
      networkPassphrase: params.network_passphrase,
    })
      .addOperation(contract.call('root'))
      .setTimeout(30)
      .build();
    const sim = await srv.simulateTransaction(built);
    if (rpc.Api.isSimulationError(sim) || !sim.result?.retval) return null;
    const bytes = scValToNative(sim.result.retval) as Uint8Array;
    let hex = '';
    for (const b of bytes) hex += b.toString(16).padStart(2, '0');
    return '0x' + hex;
  } catch {
    return null;
  }
}
