use crate::types::Address;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;

pub struct KeyPair {
    pub public: Address,
    pub secret: [u8; 32],
}

pub fn generate_keypair() -> KeyPair {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let public = verifying_key.to_bytes();
    let secret = signing_key.to_bytes();
    KeyPair { public, secret }
}

pub fn address_from_public_key(public_key: &[u8; 32]) -> Address {
    *public_key
}

pub fn load_keypair(secret_bytes: &[u8; 32]) -> Option<KeyPair> {
    let signing_key = SigningKey::from_bytes(secret_bytes);
    let verifying_key = signing_key.verifying_key();
    Some(KeyPair {
        public: verifying_key.to_bytes(),
        secret: *secret_bytes,
    })
}

pub fn sign_block(secret: &[u8; 32], block_hash: &[u8; 32]) -> [u8; 64] {
    let signing_key = SigningKey::from_bytes(secret);
    signing_key.sign(block_hash).to_bytes()
}

pub fn verify_block_signature(
    public: &Address,
    block_hash: &[u8; 32],
    signature: &[u8; 64],
) -> bool {
    let verifying_key = match VerifyingKey::from_bytes(public) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = match ed25519_dalek::Signature::from_slice(signature) {
        Ok(s) => s,
        Err(_) => return false,
    };
    verifying_key.verify_strict(block_hash, &sig).is_ok()
}
