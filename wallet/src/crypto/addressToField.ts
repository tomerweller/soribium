// Compress a Soroban address (56-char strkey) into one field element —
// mirror of harness/src/l1.rs: the ASCII strkey split into two 28-byte
// big-endian limbs (28 < 32 bytes, so both are canonical field elements).
import { Fr } from './fields';
import { DOMAIN_ADDR, p2 } from './poseidon2';

export function addressToField(strkey: string): Fr {
  if (strkey.length !== 56) {
    throw new Error(`addressToField: expected 56-char strkey, got ${strkey.length}`);
  }
  let limb0 = 0n;
  let limb1 = 0n;
  for (let i = 0; i < 28; i++) {
    const c0 = strkey.charCodeAt(i);
    const c1 = strkey.charCodeAt(i + 28);
    if (c0 > 0x7f || c1 > 0x7f) {
      throw new Error('addressToField: non-ASCII strkey');
    }
    limb0 = (limb0 << 8n) | BigInt(c0);
    limb1 = (limb1 << 8n) | BigInt(c1);
  }
  return p2([DOMAIN_ADDR, limb0, limb1]);
}
