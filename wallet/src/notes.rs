use ff::PrimeField;
use poseidon_rs::{Fr, Poseidon};

pub struct Note {
    pub value: u64,
    pub salt: u64,
    pub owner: u64,
    pub asset_id: u64,
    pub maturity_date: u64, // Unix timestamp
}

impl Note {
    pub fn commit(&self) -> Fr {
        let f_val = Fr::from_str(&self.value.to_string()).unwrap();

        let f_owner = Fr::from_str(&self.owner.to_string()).unwrap();

        let f_salt = Fr::from_str(&self.salt.to_string()).expect("Salt too large for field?");

        let f_asset = Fr::from_str(&self.asset_id.to_string()).unwrap();

        let f_maturity_date = Fr::from_str(&self.maturity_date.to_string()).unwrap();

        let hasher = Poseidon::new();
        hasher
            .hash(vec![f_val, f_salt, f_owner, f_asset, f_maturity_date])
            .unwrap()
    }

    pub fn nullifer(&self, private_key: Fr) -> Fr {
        let f_salt = Fr::from_str(&self.salt.to_string()).expect("Salt too large for field?");

        let hasher = Poseidon::new();
        hasher.hash(vec![f_salt, private_key]).unwrap()
    }
}
