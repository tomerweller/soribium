use soroban_sdk::{contractevent, BytesN};

#[contractevent(topics = ["deposit"], data_format = "map")]
pub struct Deposit<'a> {
    #[topic]
    pub seq: &'a u64,
    pub pk_x: &'a BytesN<32>,
    pub amount: &'a i128,
}

#[contractevent(topics = ["batch"], data_format = "map")]
pub struct Batch<'a> {
    #[topic]
    pub batch_num: &'a u64,
    pub new_root: &'a BytesN<32>,
    /// DA commitment (5th public input) so external verifiers can audit
    /// blob availability without fetching the envelope.
    pub da_commitment: &'a BytesN<32>,
}
