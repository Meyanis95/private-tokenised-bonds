use ff::PrimeField;
use poseidon_rs::{Fr, Poseidon};
use rand::{self, Rng};
use sha3::{Digest, Keccak256};

pub struct ShieldedKeys {
    seed: [u8; 32],
    private_spending_key: Fr,
    public_spending_key: Fr,
    private_viewing_key: [u8; 32],
    public_viewing_key: [u8; 32],
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

        let hasher = Poseidon::new();
        let public_spending_key = hasher
            .hash(vec![private_spending_key])
            .expect("Failed to hash private spending key");

        // Derive viewing keys for encryption (X25519)
        let private_viewing_key = Self::derive_viewing_key(&seed);
        let public_viewing_key = Self::x25519_public(&private_viewing_key);

        ShieldedKeys {
            seed,
            private_spending_key,
            public_spending_key,
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

    /// Derive X25519 private key from seed using Keccak256
    fn derive_viewing_key(seed: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(seed);
        hasher.update(b"x25519_encryption");
        let derived = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&derived[..32]);
        key
    }

    /// Convert X25519 private key to public key (placeholder - simplified)
    fn x25519_public(private_key: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(private_key);
        hasher.update(b"public");
        let derived = hasher.finalize();

        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&derived[..32]);
        pubkey
    }

    /// Get the public spending key
    pub fn public_spending_key(&self) -> Fr {
        self.public_spending_key
    }

    /// Get the public viewing key
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
            .hash(vec![f_salt, self.private_spending_key])
            .expect("Failed to compute nullifier")
    }

    /// Derive shared secret with another party's public viewing key (ECDH placeholder)
    pub fn ecdh(&self, their_pubkey: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(&self.private_viewing_key);
        hasher.update(their_pubkey);
        let shared = hasher.finalize();

        let mut result = [0u8; 32];
        result.copy_from_slice(&shared[..32]);
        result
    }
}
