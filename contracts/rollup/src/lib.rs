//! Payments ZK-rollup spike contract: custody of exactly one SEP-41 token,
//! state root advanced by UltraHonk-proven batches (DESIGN.md).
#![no_std]

pub mod events;
pub mod publics;
pub mod storage;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env, Vec,
};
pub use storage::PendingDeposit;
use ultrahonk_soroban_verifier::{UltraHonkVerifier, PROOF_BYTES};

/// Inline withdrawal execution cap per batch (write-entry/CPU headroom).
pub const MAX_WITHDRAWALS: u32 = 8;
/// L2 balances are u64 in-circuit; deposits must fit.
pub const MAX_AMOUNT: i128 = (u64::MAX as i128) + 1;

#[contracterror]
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RollupError {
    InvalidVerificationKey = 1,
    InvalidProofLength = 2,
    VerificationFailed = 3,
    InvalidAmount = 4,
    NonCanonicalField = 5,
    TooManyWithdrawals = 6,
    NotEnoughDeposits = 7,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BatchEnvelope {
    pub new_root: BytesN<32>,
    /// How many entries of the FIFO deposit queue this batch consumes.
    pub deposit_count: u32,
    pub withdrawals: Vec<Withdrawal>,
    /// Poseidon2 fold over the batch's tx messages (DOMAIN_DA), proven
    /// in-circuit as the 5th public input. Validium: the blob itself lives
    /// off-chain (sequencer DA endpoint); verifiers re-fold it against this
    /// commitment.
    pub da_commitment: BytesN<32>,
    pub proof: Bytes,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Withdrawal {
    pub dest: Address,
    pub amount: i128,
}

#[contract]
pub struct RollupContract;

#[contractimpl]
impl RollupContract {
    pub fn __constructor(
        env: Env,
        token: Address,
        vk: Bytes,
        genesis_root: BytesN<32>,
    ) -> Result<(), RollupError> {
        // Parse-validate the VK. 5 user PIs (old_root, new_root,
        // deposit_hash, withdraw_hash, da_commitment) + 16 pairing.
        UltraHonkVerifier::new(&env, &vk).map_err(|_| RollupError::InvalidVerificationKey)?;
        storage::set_vk(&env, &vk);
        storage::set_token(&env, &token);
        storage::set_root(&env, &genesis_root);
        Ok(())
    }

    /// Escrow `amount` of the pinned token and enqueue an L2 credit to the
    /// account with public key x-coordinate `l2_pk_x`. The credit lands when
    /// a batch consumes the queue entry.
    pub fn deposit(
        env: Env,
        from: Address,
        l2_pk_x: BytesN<32>,
        amount: i128,
    ) -> Result<u64, RollupError> {
        from.require_auth();
        if amount <= 0 || amount >= MAX_AMOUNT {
            return Err(RollupError::InvalidAmount);
        }
        let arr = l2_pk_x.to_array();
        if !publics::is_canonical_field(&arr) || arr == publics::FR_ZERO_WORD {
            return Err(RollupError::NonCanonicalField);
        }

        let token_client = token::TokenClient::new(&env, &storage::get_token(&env));
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        let seq = storage::enqueue_deposit(&env, &PendingDeposit { pk_x: l2_pk_x.clone(), amount });
        events::Deposit { seq: &seq, pk_x: &l2_pk_x, amount: &amount }.publish(&env);
        Ok(seq)
    }

    /// Verify a batch proof against the current root and the FIFO prefix of
    /// the deposit queue; on success advance the root, release the consumed
    /// deposits, and pay out the batch's withdrawals.
    pub fn submit_batch(
        env: Env,
        sequencer: Address,
        envelope: BatchEnvelope,
    ) -> Result<(), RollupError> {
        sequencer.require_auth();

        if envelope.proof.len() as usize != PROOF_BYTES {
            return Err(RollupError::InvalidProofLength);
        }
        if envelope.withdrawals.len() > MAX_WITHDRAWALS {
            return Err(RollupError::TooManyWithdrawals);
        }
        let head = storage::dep_head(&env);
        let tail = storage::dep_tail(&env);
        let count = envelope.deposit_count as u64;
        if head + count > tail {
            return Err(RollupError::NotEnoughDeposits);
        }

        // --- assemble the 4 public inputs (128 bytes), all derived on-chain ---
        let old_root = storage::get_root(&env);

        let mut deposit_hash = BytesN::from_array(&env, &publics::FR_ZERO_WORD);
        for seq in head..head + count {
            let dep = storage::get_deposit(&env, seq);
            deposit_hash = publics::fold(&env, publics::DOMAIN_DEP, &deposit_hash, &dep.pk_x, dep.amount);
        }

        let mut withdraw_hash = BytesN::from_array(&env, &publics::FR_ZERO_WORD);
        for wd in envelope.withdrawals.iter() {
            if wd.amount <= 0 || wd.amount >= MAX_AMOUNT {
                return Err(RollupError::InvalidAmount);
            }
            let dest_field = publics::address_to_field(&env, &wd.dest);
            withdraw_hash = publics::fold(&env, publics::DOMAIN_WD, &withdraw_hash, &dest_field, wd.amount);
        }

        let mut pis = Bytes::new(&env);
        publics::append_field(&env, &mut pis, &old_root);
        publics::append_field(&env, &mut pis, &envelope.new_root);
        publics::append_field(&env, &mut pis, &deposit_hash);
        publics::append_field(&env, &mut pis, &withdraw_hash);
        publics::append_field(&env, &mut pis, &envelope.da_commitment);

        // --- verify ---
        let vk = storage::get_vk(&env);
        let verifier =
            UltraHonkVerifier::new(&env, &vk).map_err(|_| RollupError::InvalidVerificationKey)?;
        verifier
            .verify(&env, &envelope.proof, &pis)
            .map_err(|_| RollupError::VerificationFailed)?;

        // --- state transition ---
        storage::dequeue_deposits(&env, count);
        storage::set_root(&env, &envelope.new_root);
        let batch_num = storage::get_batch_num(&env) + 1;
        storage::set_batch_num(&env, batch_num);

        let token_client = token::TokenClient::new(&env, &storage::get_token(&env));
        for wd in envelope.withdrawals.iter() {
            token_client.transfer(&env.current_contract_address(), &wd.dest, &wd.amount);
        }

        events::Batch {
            batch_num: &batch_num,
            new_root: &envelope.new_root,
            da_commitment: &envelope.da_commitment,
        }
        .publish(&env);
        Ok(())
    }

    /// Verify-only entrypoint kept for cost isolation on localnet (M6).
    pub fn verify(env: Env, public_inputs: Bytes, proof: Bytes) -> Result<(), RollupError> {
        if proof.len() as usize != PROOF_BYTES {
            return Err(RollupError::InvalidProofLength);
        }
        let vk = storage::get_vk(&env);
        let verifier =
            UltraHonkVerifier::new(&env, &vk).map_err(|_| RollupError::InvalidVerificationKey)?;
        verifier
            .verify(&env, &proof, &public_inputs)
            .map_err(|_| RollupError::VerificationFailed)
    }

    pub fn root(env: Env) -> BytesN<32> {
        storage::get_root(&env)
    }

    pub fn batch_num(env: Env) -> u64 {
        storage::get_batch_num(&env)
    }

    pub fn pending_deposit_count(env: Env) -> u64 {
        storage::dep_tail(&env) - storage::dep_head(&env)
    }

    /// Next unassigned deposit-queue sequence number (exclusive end).
    pub fn dep_tail(env: Env) -> u64 {
        storage::dep_tail(&env)
    }

    /// First unconsumed deposit-queue sequence number.
    pub fn dep_head(env: Env) -> u64 {
        storage::dep_head(&env)
    }

    /// Read one pending queue entry (traps if consumed/nonexistent). The
    /// sequencer's deposit watcher polls dep_tail + this instead of events:
    /// contract storage has no retention window.
    pub fn get_pending_deposit(env: Env, seq: u64) -> PendingDeposit {
        storage::get_deposit(&env, seq)
    }

    pub fn token(env: Env) -> Address {
        storage::get_token(&env)
    }
}

#[cfg(test)]
mod test;
