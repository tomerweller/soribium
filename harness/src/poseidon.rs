//! Poseidon2 for the harness, computed through a Soroban Env so it is the
//! same host implementation the contract uses (and, via the equivalence the
//! Nethermind tornado e2e depends on, the same function as noir-lang/poseidon
//! in-circuit).

use soroban_poseidon::poseidon2_hash;
use soroban_sdk::{crypto::BnScalar, Bytes, Env, U256, Vec as SVec};

/// A BN254 scalar-field element, 32-byte big-endian canonical encoding.
pub type Fr = [u8; 32];

pub const FR_ZERO: Fr = [0u8; 32];

pub fn fr_from_u64(v: u64) -> Fr {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

pub struct Hasher {
    env: Env,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher {
    pub fn new() -> Self {
        let env = Env::default();
        env.cost_estimate().budget().reset_unlimited();
        Self { env }
    }

    pub fn hash(&self, inputs: &[Fr]) -> Fr {
        let mut v: SVec<U256> = SVec::new(&self.env);
        for word in inputs {
            let bytes = Bytes::from_array(&self.env, word);
            v.push_back(U256::from_be_bytes(&self.env, &bytes));
        }
        let out = poseidon2_hash::<4, BnScalar>(&self.env, &v);
        let mut arr = [0u8; 32];
        out.to_be_bytes().copy_into_slice(&mut arr);
        arr
    }

    pub fn hash2(&self, a: Fr, b: Fr) -> Fr {
        self.hash(&[a, b])
    }
}

pub fn to_hex(fr: &Fr) -> String {
    let mut s = String::with_capacity(66);
    s.push_str("0x");
    for b in fr {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
