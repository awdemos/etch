use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];
pub type Address = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload(pub [u8; 256]);

impl Serialize for Payload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Payload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        if bytes.len() != 256 {
            return Err(serde::de::Error::custom("payload must be 256 bytes"));
        }
        let mut arr = [0u8; 256];
        arr.copy_from_slice(&bytes);
        Ok(Payload(arr))
    }
}

pub const MAX_PAYLOADS_PER_BLOCK: usize = 1024;
pub const TARGET_BLOCK_TIME_SECS: u64 = 120;
pub const BLOCKS_PER_YEAR: u64 = 262_800;
pub const DIFFICULTY_ADJUSTMENT_PERIOD: u64 = BLOCKS_PER_YEAR;
pub const DIFFICULTY_VOTE_WINDOW: u64 = 1_000;
pub const BLOCK_REWARD: u64 = 500_000_000;
pub const REWARD_DECIMALS: u32 = 9;
pub const MAX_SUPPLY: u64 = 21_000_000_000_000_000;
pub const SCRYPT_N: u64 = 32_768;
pub const SCRYPT_R: u32 = 8;
pub const SCRYPT_P: u32 = 1;
pub const SCRYPT_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub previous_hash: Hash,
    pub timestamp: u64,
    pub miner_address: Address,
    pub difficulty_vote: i8,
    pub payload_count: u32,
    pub merkle_root: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub nonce: u64,
    pub payloads: Vec<Payload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Difficulty {
    pub target: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub genesis_timestamp: u64,
    pub genesis_difficulty: Difficulty,
    pub genesis_payload: Payload,
}

impl Block {
    pub fn hash(&self) -> Hash {
        let bytes = postcard::to_stdvec(&self.header).expect("serialize");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hasher.update(&self.nonce.to_le_bytes());
        hasher.finalize().into()
    }

    pub fn pow_hash(&self) -> Hash {
        let block_hash = self.hash();
        let mut output = [0u8; 32];
        let params = scrypt::Params::new(
            SCRYPT_N.trailing_zeros() as u8,
            SCRYPT_R,
            SCRYPT_P,
            SCRYPT_LEN,
        )
        .expect("scrypt params");
        scrypt::scrypt(&block_hash, &block_hash, &params, &mut output).expect("scrypt hash");
        output
    }

    pub fn verify_payload_count(&self) -> bool {
        self.header.payload_count as usize == self.payloads.len()
            && self.payloads.len() <= MAX_PAYLOADS_PER_BLOCK
    }

    pub fn compute_merkle_root(payloads: &[Payload]) -> Hash {
        if payloads.is_empty() {
            return [0u8; 32];
        }
        let mut hashes: Vec<Hash> = payloads
            .iter()
            .map(|p| {
                let mut hasher = Sha256::new();
                hasher.update(&p.0);
                hasher.finalize().into()
            })
            .collect();
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in hashes.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(&chunk[0]);
                if chunk.len() == 2 {
                    hasher.update(&chunk[1]);
                } else {
                    hasher.update(&chunk[0]);
                }
                next_level.push(hasher.finalize().into());
            }
            hashes = next_level;
        }
        hashes[0]
    }

    pub fn verify_merkle_root(&self) -> bool {
        self.header.merkle_root == Self::compute_merkle_root(&self.payloads)
    }
}

impl Difficulty {
    pub fn new(target: Hash) -> Self {
        Self { target }
    }

    pub fn genesis() -> Self {
        let mut target = [0u8; 32];
        target[0] = 0x00;
        target[1] = 0x00;
        target[2] = 0x0F;
        target[3] = 0xFF;
        Self::new(target)
    }

    pub fn satisfies(&self, hash: &Hash) -> bool {
        hash <= &self.target
    }
}

impl ChainConfig {
    pub fn mainnet() -> Self {
        Self {
            genesis_timestamp: 1_700_000_000,
            genesis_difficulty: Difficulty::genesis(),
            genesis_payload: Payload([0u8; 256]),
        }
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self::mainnet()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_hash_deterministic() {
        let block = Block {
            header: BlockHeader {
                previous_hash: [1u8; 32],
                timestamp: 1000,
                miner_address: [2u8; 32],
                difficulty_vote: 0,
                payload_count: 0,
                merkle_root: [0u8; 32],
            },
            nonce: 42,
            payloads: vec![],
        };
        let h1 = block.hash();
        let h2 = block.hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_merkle_root_empty() {
        let root = Block::compute_merkle_root(&[]);
        assert_eq!(root, [0u8; 32]);
    }

    #[test]
    fn test_merkle_root_single() {
        let payload = Payload([1u8; 256]);
        let root = Block::compute_merkle_root(&[payload]);
        let mut hasher = sha2::Sha256::new();
        hasher.update(&payload.0);
        let expected: Hash = hasher.finalize().into();
        assert_eq!(root, expected);
    }

    #[test]
    fn test_difficulty_satisfies() {
        let diff = Difficulty::genesis();
        let mut hash = [0u8; 32];
        hash[0] = 0x00;
        hash[1] = 0x00;
        hash[2] = 0x0F;
        hash[3] = 0xFE;
        assert!(diff.satisfies(&hash));
        let mut hash2 = [0u8; 32];
        hash2[0] = 0xFF;
        assert!(!diff.satisfies(&hash2));
    }

    #[test]
    fn test_payload_serde() {
        let payload = Payload([42u8; 256]);
        let bytes = postcard::to_stdvec(&payload).unwrap();
        let decoded: Payload = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(payload.0, decoded.0);
    }

    #[test]
    fn test_block_serde() {
        let block = Block {
            header: BlockHeader {
                previous_hash: [1u8; 32],
                timestamp: 1000,
                miner_address: [2u8; 32],
                difficulty_vote: 1,
                payload_count: 1,
                merkle_root: [3u8; 32],
            },
            nonce: 12345,
            payloads: vec![Payload([5u8; 256])],
        };
        let bytes = postcard::to_stdvec(&block).unwrap();
        let decoded: Block = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(block.hash(), decoded.hash());
    }
}
