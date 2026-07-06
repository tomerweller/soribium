// Recursion benchmark: verify ONE inner UltraHonk proof (batch_n16,
// poseidon2 transcript) inside a circuit. Measures the fixed per-proof
// aggregation overhead for a rollup proof tree. proof_type 1 = HONK
// (barretenberg PROOF_TYPE enum).
fn main(
    verification_key: [Field; 112],
    proof: [Field; 456],
    public_inputs: pub [Field; 5],
    key_hash: Field,
) {
    std::verify_proof_with_type(verification_key, proof, public_inputs, key_hash, 1);
}
