use ff::*;
use poseidon_rs::Fr;

use crate::merkle::MerkleTree;
use crate::notes::Note;

mod merkle;
mod notes;

fn main() {
    let input_note = Note {
        value: 100,
        salt: 0,
        owner_public_key: 10,
        asset_id: 0,
    };

    let output_note = Note {
        value: 80,
        salt: 0,
        owner_public_key: 10,
        asset_id: 0,
    };

    println!("output note commitment: {:?}", output_note.commit());
    println!(
        "note nullifier: {:?}",
        input_note.nullifer(Fr::from_raw_repr(10.into()).unwrap())
    );

    let state = [
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
        input_note.commit(),
    ];

    let mt = MerkleTree::new(&state);
    let proof = mt.generate_proof(2);

    println!("proof: {:?}", mt.levels);
    println!("proof: {:?}", proof);
}
