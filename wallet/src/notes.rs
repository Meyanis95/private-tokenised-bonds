use ff::PrimeField;
use poseidon_rs::{Fr, Poseidon};
use std::str::FromStr;

// TODO: Define proper PublicKey and PrivateKey types
type PublicKey = [u8; 32];
type PrivateKey = Fr;

pub struct Note<F> {
    pub value: F,
    pub salt: F,
    pub owner: F,
    pub asset_id: F,
    pub maturity_date: F,
}

impl<F: ToString> Note<F> {
    pub fn commit(&self) -> Fr {
        let f_val = Fr::from_str(&self.value.to_string()).unwrap();

        // let owner_hex = hex::encode(self.owner_public_key);
        // let owner_big = BigUint::parse_bytes(owner_hex.as_bytes(), 16).unwrap();
        let f_owner = Fr::from_str(&self.owner.to_string()).unwrap();

        // let salt_hex = hex::encode(self.salt);
        // let salt_big = BigUint::parse_bytes(self.salt, 16).unwrap();
        let f_salt = Fr::from_str(&self.salt.to_string()).expect("Salt too large for field?");

        let f_asset = Fr::from_str(&self.asset_id.to_string()).unwrap();

        let f_maturity_date = Fr::from_str(&self.maturity_date.to_string()).unwrap();

        let hasher = Poseidon::new();
        hasher
            .hash(vec![f_val, f_salt, f_owner, f_asset, f_maturity_date])
            .unwrap()
    }

    pub fn nullifer(&self, private_key: PrivateKey) -> Fr {
        // let owner_hex = hex::encode(private_key);
        // let owner_big = BigUint::parse_bytes(owner_hex.as_bytes(), 16).unwrap();
        // let f_owner = Fr::from_str(&owner_big.to_string()).unwrap();

        // let salt_hex = hex::encode(self.salt);
        // let salt_big = BigUint::parse_bytes(salt_hex.as_bytes(), 16).unwrap();
        let f_salt = Fr::from_str(&self.salt.to_string()).expect("Salt too large for field?");

        let hasher = Poseidon::new();
        hasher.hash(vec![f_salt, private_key]).unwrap()
    }
}
