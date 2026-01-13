use ff::PrimeField;
use poseidon_rs::{Fr, Poseidon};
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShieldedKeys {
    seed: [u8; 32],
    private_spending_key_hex: String,
    pub public_spending_key_hex: String,
    private_viewing_key: [u8; 32],
    pub public_viewing_key: [u8; 32],
}

impl ShieldedKeys {
    /// Generate new shielded keys from a random seed
    pub fn generate() -> Self {
        let seed = rand::thread_rng().gen::<[u8; 32]>();
        Self::from_seed(seed)
    }

    /// Derive shielded keys from a seed
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let private_spending_key = Self::derive_spending_key(&seed);
        let private_spending_key_hex = format!("{}", private_spending_key);

        let hasher = Poseidon::new();
        let public_spending_key = hasher
            .hash(vec![private_spending_key])
            .expect("Failed to hash private spending key");
        let public_spending_key_hex = format!("{}", public_spending_key);

        // Derive viewing keys for encryption (X25519)
        let (private_viewing_key, public_viewing_key) = Self::derive_public_viewing_key(&seed);

        ShieldedKeys {
            seed,
            private_spending_key_hex,
            public_spending_key_hex,
            private_viewing_key,
            public_viewing_key,
        }
    }

    /// Derive spending keys from seed using Keccak256
    fn derive_spending_key(seed: &[u8; 32]) -> Fr {
        let mut hasher = Keccak256::new();
        hasher.update(seed);
        hasher.update(b"spending_key");
        let derived = hasher.finalize();

        // Convert first 8 bytes to u64 then to string for Fr::from_str
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&derived[..8]);
        let num = u64::from_le_bytes(bytes);
        Fr::from_str(&num.to_string()).expect("Failed to create Fr from derived key")
    }

    /// Derive X25519 private key from seed and return public key
    fn derive_public_viewing_key(seed: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
        let viewing_secret = StaticSecret::from(*seed);
        let viewing_public = PublicKey::from(&viewing_secret);
        (*viewing_secret.as_bytes(), *viewing_public.as_bytes())
    }

    /// Reconstruct the private spending key Fr from stored hex
    fn get_private_spending_key(&self) -> Fr {
        Fr::from_str(&self.private_spending_key_hex)
            .expect("Failed to parse stored private spending key")
    }

    /// Reconstruct the public spending key Fr from stored hex
    fn get_public_spending_key(&self) -> Fr {
        Fr::from_str(&self.public_spending_key_hex)
            .expect("Failed to parse stored public spending key")
    }

    /// Reconstruct the private viewing key StaticSecret from seed
    fn get_private_viewing_key(&self) -> StaticSecret {
        StaticSecret::from(self.seed)
    }

    /// Get the public spending key
    pub fn public_spending_key(&self) -> Fr {
        self.get_public_spending_key()
    }

    /// Get the public viewing key as bytes
    pub fn public_viewing_key(&self) -> &[u8; 32] {
        &self.public_viewing_key
    }

    /// Get the seed (for storage)
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }

    /// Sign a message (nullifier) using the private spending key
    pub fn sign_nullifier(&self, salt: u64) -> Fr {
        let f_salt = Fr::from_str(&salt.to_string()).expect("Salt conversion failed");
        let hasher = Poseidon::new();
        hasher
            .hash(vec![f_salt, self.get_private_spending_key()])
            .expect("Failed to compute nullifier")
    }

    /// Derive shared secret with another party's public viewing key (ECDH)
    pub fn ecdh(&self, their_pubkey: &[u8; 32]) -> [u8; 32] {
        let other_party_public_key = PublicKey::from(*their_pubkey);
        let private_viewing = self.get_private_viewing_key();
        let shared_secret = private_viewing.diffie_hellman(&other_party_public_key);
        shared_secret.to_bytes()
    }
}
