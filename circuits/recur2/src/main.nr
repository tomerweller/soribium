// Verify TWO inner UltraHonk proofs - marginal aggregation cost.
fn main(
    verification_key: [Field; 112],
    proof_a: [Field; 456],
    pi_a: pub [Field; 5],
    proof_b: [Field; 456],
    pi_b: pub [Field; 5],
    key_hash: Field,
) {
    std::verify_proof_with_type(verification_key, proof_a, pi_a, key_hash, 1);
    std::verify_proof_with_type(verification_key, proof_b, pi_b, key_hash, 1);
}
