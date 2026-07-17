use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Token,
    Vk,
    Root,
    BatchNum,
    DepHead,
    DepTail,
    /// FIFO deposit queue entry (ring buffer by sequence number).
    Dep(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingDeposit {
    pub pk_x: BytesN<32>,
    pub amount: i128,
}

pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::Token, token);
}

pub fn get_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Token).unwrap()
}

pub fn set_vk(env: &Env, vk: &Bytes) {
    env.storage().instance().set(&DataKey::Vk, vk);
}

pub fn get_vk(env: &Env) -> Bytes {
    env.storage().instance().get(&DataKey::Vk).unwrap()
}

pub fn set_root(env: &Env, root: &BytesN<32>) {
    env.storage().instance().set(&DataKey::Root, root);
}

pub fn get_root(env: &Env) -> BytesN<32> {
    env.storage().instance().get(&DataKey::Root).unwrap()
}

pub fn set_batch_num(env: &Env, n: u64) {
    env.storage().instance().set(&DataKey::BatchNum, &n);
}

pub fn get_batch_num(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::BatchNum).unwrap_or(0)
}

pub fn dep_head(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::DepHead).unwrap_or(0)
}

pub fn dep_tail(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::DepTail).unwrap_or(0)
}

pub fn enqueue_deposit(env: &Env, dep: &PendingDeposit) -> u64 {
    let tail = dep_tail(env);
    env.storage().persistent().set(&DataKey::Dep(tail), dep);
    env.storage().instance().set(&DataKey::DepTail, &(tail + 1));
    tail
}

pub fn get_deposit(env: &Env, seq: u64) -> PendingDeposit {
    env.storage().persistent().get(&DataKey::Dep(seq)).unwrap()
}

pub fn dequeue_deposits(env: &Env, count: u64) {
    let head = dep_head(env);
    for seq in head..head + count {
        env.storage().persistent().remove(&DataKey::Dep(seq));
    }
    env.storage().instance().set(&DataKey::DepHead, &(head + count));
}
