use crate::types::{Block, Difficulty, Hash, MAX_PAYLOADS_PER_BLOCK, TARGET_BLOCK_TIME_SECS, DIFFICULTY_VOTE_WINDOW, DIFFICULTY_ADJUSTMENT_PERIOD, BLOCK_REWARD, MAX_SUPPLY};
use crate::storage::BlockStorage;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Consensus;

impl Consensus {
    pub fn validate_block(block: &Block, storage: &BlockStorage, expected_difficulty: &Difficulty) -> Result<(), String> {
        if !block.verify_payload_count() {
            return Err("payload count mismatch".to_string());
        }
        if !block.verify_merkle_root() {
            return Err("merkle root mismatch".to_string());
        }
        if block.header.previous_hash != [0u8; 32] {
            let pow_hash = block.pow_hash();
            if !expected_difficulty.satisfies(&pow_hash) {
                return Err("insufficient proof of work".to_string());
            }
        }
        if block.payloads.len() > MAX_PAYLOADS_PER_BLOCK {
            return Err("too many payloads".to_string());
        }
        let prev_hash = block.header.previous_hash;
        if prev_hash != [0u8; 32] && !storage.has_block(&prev_hash) {
            return Err("previous block not found".to_string());
        }
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if block.header.timestamp > current_time + TARGET_BLOCK_TIME_SECS {
            return Err("timestamp too far in future".to_string());
        }
        if let Some(prev_block) = storage.get_block(&prev_hash) {
            if block.header.timestamp <= prev_block.header.timestamp {
                return Err("timestamp not increasing".to_string());
            }
        }
        Ok(())
    }

    pub fn compute_difficulty(storage: &BlockStorage, height: u64, parent_hash: &Hash, genesis_difficulty: &Difficulty) -> Difficulty {
        let _parent = match storage.get_block(parent_hash) {
            Some(b) => b,
            None => return genesis_difficulty.clone(),
        };
        let parent_height = storage.height_of(parent_hash).unwrap_or(0);
        let parent_difficulty = Self::difficulty_at_height(storage, parent_height, genesis_difficulty);
        if height % DIFFICULTY_ADJUSTMENT_PERIOD != 0 {
            return parent_difficulty;
        }
        let votes = Self::collect_votes(storage, parent_height);
        let mut up = 0u64;
        let mut down = 0u64;
        for vote in votes {
            if vote > 0 { up += 1; }
            else if vote < 0 { down += 1; }
        }
        let mut target = parent_difficulty.target;
        if up > DIFFICULTY_VOTE_WINDOW / 2 {
            target = Self::adjust_target(&target, 90);
        } else if down > DIFFICULTY_VOTE_WINDOW / 2 {
            target = Self::adjust_target(&target, 110);
        }
        Difficulty::new(target)
    }

    pub fn difficulty_at_height(storage: &BlockStorage, height: u64, genesis_difficulty: &Difficulty) -> Difficulty {
        if height == 0 {
            return genesis_difficulty.clone();
        }
        let block = match storage.get_block_by_height(height) {
            Some(b) => b,
            None => return genesis_difficulty.clone(),
        };
        let _hash = block.hash();
        Self::compute_difficulty(storage, height, &block.header.previous_hash, genesis_difficulty)
    }

    fn collect_votes(storage: &BlockStorage, up_to_height: u64) -> Vec<i8> {
        let start = up_to_height.saturating_sub(DIFFICULTY_VOTE_WINDOW);
        let mut votes = Vec::new();
        for h in start..up_to_height {
            if let Some(block) = storage.get_block_by_height(h) {
                votes.push(block.header.difficulty_vote);
            }
        }
        votes
    }

    fn adjust_target(target: &Hash, percent: u64) -> Hash {
        let mut result = [0u8; 32];
        let mut carry = 0u64;
        for i in (0..32).rev() {
            let val = (target[i] as u64) * percent + carry;
            result[i] = (val / 100) as u8;
            carry = val % 100;
        }
        result
    }

    pub fn cumulative_work(storage: &BlockStorage, tip_hash: &Hash, genesis_difficulty: &Difficulty) -> u128 {
        let mut work = 0u128;
        let mut current = *tip_hash;
        loop {
            let height = match storage.height_of(&current) {
                Some(h) => h,
                None => break,
            };
            let diff = Self::difficulty_at_height(storage, height, genesis_difficulty);
            work += Self::block_work(&diff);
            if height == 0 {
                break;
            }
            let block = match storage.get_block(&current) {
                Some(b) => b,
                None => break,
            };
            current = block.header.previous_hash;
        }
        work
    }

    fn block_work(difficulty: &Difficulty) -> u128 {
        let target_val = u128::from_be_bytes([
            difficulty.target[0], difficulty.target[1], difficulty.target[2], difficulty.target[3],
            difficulty.target[4], difficulty.target[5], difficulty.target[6], difficulty.target[7],
            difficulty.target[8], difficulty.target[9], difficulty.target[10], difficulty.target[11],
            difficulty.target[12], difficulty.target[13], difficulty.target[14], difficulty.target[15],
        ]);
        if target_val == 0 {
            return u128::MAX;
        }
        u128::MAX / target_val
    }

    pub fn select_best_chain<'a>(storage: &BlockStorage, candidates: &[Hash], genesis_difficulty: &Difficulty) -> Option<Hash> {
        candidates.iter()
            .max_by_key(|hash| Self::cumulative_work(storage, hash, genesis_difficulty))
            .copied()
    }

    pub fn get_ancestors(storage: &BlockStorage, hash: &Hash) -> Vec<Hash> {
        let mut ancestors = Vec::new();
        let mut current = *hash;
        loop {
            let block = match storage.get_block(&current) {
                Some(b) => b,
                None => break,
            };
            ancestors.push(current);
            if block.header.previous_hash == [0u8; 32] {
                break;
            }
            current = block.header.previous_hash;
        }
        ancestors
    }

    pub fn find_common_ancestor(storage: &BlockStorage, a: &Hash, b: &Hash) -> Option<Hash> {
        let ancestors_a: std::collections::HashSet<Hash> = Self::get_ancestors(storage, a).into_iter().collect();
        let ancestors_b = Self::get_ancestors(storage, b);
        for hash in ancestors_b {
            if ancestors_a.contains(&hash) {
                return Some(hash);
            }
        }
        None
    }

    pub fn verify_supply(storage: &BlockStorage) -> bool {
        let mut total = 0u64;
        for height in 0..=storage.tip().1 {
            if let Some(_) = storage.get_block_by_height(height) {
                total += BLOCK_REWARD;
                if total > MAX_SUPPLY {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BlockStorage;
    use crate::types::{Block, BlockHeader};
    use tempfile::TempDir;

    fn make_block(prev_hash: Hash, timestamp: u64, nonce: u64) -> Block {
        Block {
            header: BlockHeader {
                previous_hash: prev_hash,
                timestamp,
                miner_address: [0u8; 32],
                difficulty_vote: 0,
                payload_count: 0,
                merkle_root: [0u8; 32],
            },
            nonce,
            payloads: vec![],
        }
    }

    #[test]
    fn test_validate_genesis() {
        let tmp = TempDir::new().unwrap();
        let storage = BlockStorage::open(tmp.path()).unwrap();
        let genesis = make_block([0u8; 32], 1_700_000_000, 0);
        let diff = Difficulty::genesis();
        let result = Consensus::validate_block(&genesis, &storage, &diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bad_pow() {
        let tmp = TempDir::new().unwrap();
        let mut storage = BlockStorage::open(tmp.path()).unwrap();
        let genesis = make_block([0u8; 32], 1_700_000_000, 0);
        let genesis_hash = genesis.hash();
        storage.store_block(&genesis, 0).unwrap();
        let mut block = make_block(genesis_hash, 1_700_000_010, 0);
        block.header.miner_address = [1u8; 32];
        let diff = Difficulty::genesis();
        let result = Consensus::validate_block(&block, &storage, &diff);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_future_timestamp() {
        let tmp = TempDir::new().unwrap();
        let storage = BlockStorage::open(tmp.path()).unwrap();
        let mut block = make_block([0u8; 32], u64::MAX, 0);
        block.header.miner_address = [0u8; 32];
        let diff = Difficulty::genesis();
        let result = Consensus::validate_block(&block, &storage, &diff);
        assert!(result.is_err());
    }

    #[test]
    fn test_cumulative_work_genesis() {
        let tmp = TempDir::new().unwrap();
        let storage = BlockStorage::open(tmp.path()).unwrap();
        let genesis = make_block([0u8; 32], 1_700_000_000, 0);
        let hash = genesis.hash();
        let mut storage = storage;
        storage.store_block(&genesis, 0).unwrap();
        let work = Consensus::cumulative_work(&storage, &hash, &Difficulty::genesis());
        assert!(work > 0);
    }

    #[test]
    fn test_adjust_target_increase() {
        let target = [0x00u8, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let new_target = Consensus::adjust_target(&target, 90);
        assert!(new_target < target);
    }

    #[test]
    fn test_adjust_target_decrease() {
        let target = [0x00u8, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let new_target = Consensus::adjust_target(&target, 110);
        assert!(new_target > target);
    }
}
