#![no_std]

pub mod publics;
pub mod storage;

use soroban_sdk::{contract, contracterror, contractimpl, Bytes, Env};
use ultrahonk_soroban_verifier::{UltraHonkVerifier, PROOF_BYTES};

#[contracterror]
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RollupError {
    InvalidVerificationKey = 1,
    InvalidProofLength = 2,
    VerificationFailed = 3,
}

/// Spike scaffold: verification-only entrypoints. The rollup state machine
/// (deposits, batches, withdrawals) lands in M5.
#[contract]
pub struct RollupContract;

#[contractimpl]
impl RollupContract {
    pub fn __constructor(env: Env, vk: Bytes) -> Result<(), RollupError> {
        UltraHonkVerifier::new(&env, &vk).map_err(|_| RollupError::InvalidVerificationKey)?;
        storage::set_vk(&env, &vk);
        Ok(())
    }

    /// Verify a proof against the stored VK. M1 checkpoint entrypoint; also
    /// used on localnet to measure verification cost in isolation.
    pub fn verify(env: Env, public_inputs: Bytes, proof: Bytes) -> Result<(), RollupError> {
        if proof.len() as usize != PROOF_BYTES {
            return Err(RollupError::InvalidProofLength);
        }
        let vk = storage::get_vk(&env);
        let verifier = UltraHonkVerifier::new(&env, &vk)
            .map_err(|_| RollupError::InvalidVerificationKey)?;
        verifier
            .verify(&env, &proof, &public_inputs)
            .map_err(|_| RollupError::VerificationFailed)
    }
}

#[cfg(test)]
mod test;
