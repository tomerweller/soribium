// The L2 send pipeline: derive the message, sign with the active key, verify
// locally (refusing to POST a signature the circuit would reject), and submit.
import { addressToField } from '../crypto/addressToField';
import { frToHex32, hexToFr } from '../crypto/fields';
import { pkFromSk } from '../crypto/grumpkin';
import { sign, txMessage, verify } from '../crypto/schnorr';
import { api, TxRequest, WireSig } from './sequencer';

export interface SendParams {
  sk: bigint;
  /** Recipient L2 pk_x hex (transfer) or Stellar strkey (withdrawal). */
  to: string;
  amount: bigint;
  nonce: bigint;
  isWithdraw: boolean;
}

export async function signAndSubmit(p: SendParams): Promise<{ id: number; status: string }> {
  const pk = pkFromSk(p.sk);
  const toField = p.isWithdraw ? addressToField(p.to) : hexToFr(p.to);
  const msg = txMessage(pk.x, toField, p.amount, p.nonce, p.isWithdraw);
  const sig = sign(p.sk, msg);

  // Never let a bad signature leave the wallet — the sequencer would reject
  // it, but this catches wallet-side crypto bugs immediately.
  if (!verify(pk.x, pk.y, msg, sig)) {
    throw new Error('local signature verification failed — refusing to submit');
  }

  const wireSig: WireSig = {
    r_x: frToHex32(sig.r_x),
    r_y: frToHex32(sig.r_y),
    s_lo: frToHex32(sig.s_lo),
    s_hi: frToHex32(sig.s_hi),
  };
  const tx: TxRequest = {
    from_pk_x: frToHex32(pk.x),
    from_pk_y: frToHex32(pk.y),
    to: p.to,
    amount: p.amount.toString(),
    nonce: Number(p.nonce),
    is_withdraw: p.isWithdraw,
    sig: wireSig,
  };
  return api.submitTx(tx);
}
