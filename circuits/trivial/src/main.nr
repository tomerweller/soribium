// M1 sanity circuit: Poseidon2 preimage knowledge. Mirrors the Nethermind
// `identity` circuit (the one with the published ~81M-instruction on-chain
// verification benchmark). The hash is a public *return* value so witness
// generation never needs the digest precomputed.
use dep::poseidon::poseidon2::Poseidon2;

fn main(preimage: Field) -> pub Field {
    Poseidon2::hash([preimage], 1)
}

#[test]
fn hashes_preimage() {
    let h = main(42);
    assert(h != 0);
}
