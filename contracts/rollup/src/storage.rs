use soroban_sdk::{symbol_short, Bytes, Env, Symbol};

fn key_vk() -> Symbol {
    symbol_short!("vk")
}

pub fn set_vk(env: &Env, vk: &Bytes) {
    env.storage().instance().set(&key_vk(), vk);
}

pub fn get_vk(env: &Env) -> Bytes {
    env.storage()
        .instance()
        .get(&key_vk())
        .expect("vk not initialized")
}
