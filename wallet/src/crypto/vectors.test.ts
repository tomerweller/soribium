// The crypto gate: the wallet's Poseidon2 + Grumpkin + Schnorr + encodings
// must reproduce, byte-for-byte, the vectors the circuits and harness are
// pinned against. A red suite here means the wallet would produce signatures
// the circuit rejects — so this MUST pass before any UI ships (and it runs in
// the Docker build). Expected values transcribed from:
//   circuits/lib/src/schnorr_test.nr, circuits/lib/src/test.nr,
//   fixtures/batch_n4/meta.json
import { describe, expect, it } from 'vitest';
import { addressToField } from './addressToField';
import { Fr, frToHex32, hexToFr, N_GRUMPKIN, randScalar } from './fields';
import { pkFromSk } from './grumpkin';
import { computeRoot, leafValue, verifyPath } from './merkle';
import { DOMAIN_DEP, DOMAIN_WD, p2 } from './poseidon2';
import { daFold, sign, txMessage, verify } from './schnorr';

// --- pinned constants ---
const PK7_X = hexToFr('0x0e602b9dd6a3e8d039a17f069add3f9c2a187a8f629a1de60a33a8067b9b2842');
const PK7_Y = hexToFr('0x14cc8e83df1b5cbb163bd2c94005cb0707fe570def5a165242b1c1419cb014cb');
const R_X = hexToFr('0x17e6386beac25fd11573fe37404e61d754a91ee7722dbd0c1c7fff157ed56573');
const R_Y = hexToFr('0x2be1ab9ccce96a8deeb82e85a35cc26804269db7ea57bc0e0afa3b835dfcff73');
const S_LO = hexToFr('0x000000000000000000000000000000008312af54a4f4b07d782dc23d06ef43f2');
const S_HI = hexToFr('0x000000000000000000000000000000002ed87ed8cf30832f5f7b93ee4d5afd52');

const HASH2_1_2 = hexToFr('0x038682aa1cb5ae4e0a3f13da432a95c77c5c111f6f030faf9cad641ce1ed7383');
const HASH4_1_2_3_4 = hexToFr('0x130bf204a32cac1f0ace56c78b731aa3809f06df2731ebcf6b3464a15788b1b9');
const EMPTY_ROOT_D8 = hexToFr('0x067243231eddf4222f3911defbba7705aff06ed45960b27f6f91319196ef97e1');
const LEAF_1234_100_0 = hexToFr('0x08e85e74234a7e239d871de3fbd4ffba29f93e4c5eab3e47a50abe2dad310ecc');
const ROOT_LEAF_AT_5 = hexToFr('0x185afc939981ef9614d18623d87027a99db0117e16009e87db82b7bfab7865cb');
const DA_FOLD_0_42 = hexToFr('0x04753fb4f4ed6d02388a18f3b84e56dfc5b22dcd77ec8acf81ca9257bdfae3e3');

// fixtures/batch_n4/meta.json
const ALICE_PK_X = hexToFr('0x1f2cbd75183a0cced1f2b4d1121a647b3b421baed26e463b2565b7972ae1a83c');
const BOB_PK_X = hexToFr('0x13d84dfee2bd99f26530c590cd8c011979328f1fa149234b19fea91510f94ebb');
const DEPOSIT_HASH = hexToFr('0x1f6454197a8287b5e18bda5728f961ded3d87e2e81f8fcae826b373478823307');
const WITHDRAW_HASH = hexToFr('0x2feba35af40cfd8774f9593d6bc43cc5c43ff0f8e682294b747f84c072fc9ac3');
const DA_COMMITMENT = hexToFr('0x17cb89584c030ccb05e8e304a1ed5384918ca07e1794a506137090932883d3d3');
const WD_DEST = 'CADQOBYHA4DQOBYHA4DQOBYHA4DQOBYHA4DQOBYHA4DQOBYHA4DQP5KR';

const SIG7 = { r_x: R_X, r_y: R_Y, s_lo: S_LO, s_hi: S_HI };

describe('poseidon2 vs pinned circuit constants', () => {
  it('hash2(1,2)', () => expect(p2([1n, 2n])).toBe(HASH2_1_2));
  it('hash4(1,2,3,4)', () => expect(p2([1n, 2n, 3n, 4n])).toBe(HASH4_1_2_3_4));
  it('daFold(0,42)', () => expect(daFold(0n, 42n)).toBe(DA_FOLD_0_42));
});

describe('grumpkin key derivation', () => {
  it('pk(7) matches schnorr_test.nr', () => {
    const pk = pkFromSk(7n);
    expect(pk.x).toBe(PK7_X);
    expect(pk.y).toBe(PK7_Y);
  });
  it('pk(101)/pk(202) match meta.json deposits', () => {
    expect(pkFromSk(101n).x).toBe(ALICE_PK_X);
    expect(pkFromSk(202n).x).toBe(BOB_PK_X);
  });
});

describe('schnorr signature vector (sk=7, k=13, msg=42)', () => {
  it('produces the pinned signature byte-for-byte', () => {
    const sig = sign(7n, 42n, 13n);
    expect(frToHex32(sig.r_x)).toBe(frToHex32(R_X));
    expect(frToHex32(sig.r_y)).toBe(frToHex32(R_Y));
    expect(frToHex32(sig.s_lo)).toBe(frToHex32(S_LO));
    expect(frToHex32(sig.s_hi)).toBe(frToHex32(S_HI));
  });
  it('verify accepts the vector', () => {
    expect(verify(PK7_X, PK7_Y, 42n, SIG7)).toBe(true);
  });
  it('verify rejects wrong message', () => {
    expect(verify(PK7_X, PK7_Y, 43n, SIG7)).toBe(false);
  });
  it('verify rejects tampered s', () => {
    expect(verify(PK7_X, PK7_Y, 42n, { ...SIG7, s_lo: SIG7.s_lo + 1n })).toBe(false);
  });
  it('verify rejects wrong signer', () => {
    const pk8 = pkFromSk(8n);
    expect(verify(pk8.x, pk8.y, 42n, SIG7)).toBe(false);
  });
});

describe('fold chains vs meta.json', () => {
  it('deposit_hash', () => {
    const acc1 = p2([DOMAIN_DEP, 0n, ALICE_PK_X, 1000n]);
    const acc2 = p2([DOMAIN_DEP, acc1, BOB_PK_X, 500n]);
    expect(acc2).toBe(DEPOSIT_HASH);
  });
  it('withdraw_hash (exercises addressToField)', () => {
    const wd = p2([DOMAIN_WD, 0n, addressToField(WD_DEST), 100n]);
    expect(wd).toBe(WITHDRAW_HASH);
  });
  it('da_commitment (two-tx fold through txMessage)', () => {
    const msg1 = txMessage(ALICE_PK_X, BOB_PK_X, 200n, 0n, false);
    const msg2 = txMessage(BOB_PK_X, addressToField(WD_DEST), 100n, 0n, true);
    expect(daFold(daFold(0n, msg1), msg2)).toBe(DA_COMMITMENT);
  });
});

describe('encoding + roundtrips', () => {
  it('frToHex32 pads leading zeros', () => {
    expect(frToHex32(1n)).toBe('0x' + '0'.repeat(63) + '1');
    expect(hexToFr(frToHex32(S_LO))).toBe(S_LO);
  });
  it('hexToFr rejects malformed input', () => {
    expect(() => hexToFr('0x1')).toThrow();
    expect(() => hexToFr('deadbeef')).toThrow();
  });
  it('random sign/verify roundtrips', () => {
    for (let i = 0; i < 8; i++) {
      const sk = randScalar();
      const pk = pkFromSk(sk);
      const msg = randScalar() % N_GRUMPKIN;
      const sig = sign(sk, msg);
      expect(verify(pk.x, pk.y, msg, sig)).toBe(true);
      expect(verify(pk.x, pk.y, (msg + 1n) % N_GRUMPKIN, sig)).toBe(false);
    }
  });
});

describe('merkle tree vs test.nr', () => {
  function zeroLadder(): Fr[] {
    const z: Fr[] = [0n];
    for (let i = 0; i < 8; i++) z.push(p2([z[i], z[i]]));
    return z;
  }
  it('empty depth-8 root', () => {
    expect(zeroLadder()[8]).toBe(EMPTY_ROOT_D8);
  });
  it('leaf(1234,100,0)', () => {
    expect(leafValue(1234n, 100n, 0n)).toBe(LEAF_1234_100_0);
  });
  it('empty account is zero leaf', () => {
    expect(leafValue(0n, 0n, 0n)).toBe(0n);
  });
  it('single leaf at index 5 root + verifyPath', () => {
    const z = zeroLadder();
    const siblings = z.slice(0, 8);
    const leaf = leafValue(1234n, 100n, 0n);
    expect(computeRoot(leaf, 5, siblings)).toBe(ROOT_LEAF_AT_5);
    expect(verifyPath(leaf, 5, siblings, ROOT_LEAF_AT_5)).toBe(true);
    // negatives
    expect(verifyPath(leaf, 6, siblings, ROOT_LEAF_AT_5)).toBe(false);
    expect(verifyPath(leaf, 5, siblings, EMPTY_ROOT_D8)).toBe(false);
    const tampered = [...siblings];
    tampered[0] = 1n;
    expect(verifyPath(leaf, 5, tampered, ROOT_LEAF_AT_5)).toBe(false);
  });
});
