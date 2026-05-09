use crate::types::{Block, BlockHeader, ChainConfig, Difficulty, Hash, Address, Payload, MAX_PAYLOADS_PER_BLOCK};
use crate::storage::BlockStorage;
use crate::consensus::Consensus;
use std::collections::HashMap;

pub struct Blockchain {
    storage: BlockStorage,
    config: ChainConfig,
    orphan_blocks: HashMap<Hash, Block>,
}

impl Blockchain {
    pub fn new(storage: BlockStorage, config: ChainConfig) -> Self {
        Self {
            storage,
            config,
            orphan_blocks: HashMap::new(),
        }
    }

    pub fn open(data_dir: &std::path::Path, config: ChainConfig) -> Result<Self, std::io::Error> {
        let storage = BlockStorage::open(data_dir)?;
        let mut chain = Self::new(storage, config);
        if chain.storage.tip().1 == 0 {
            chain.create_genesis()?;
        }
        Ok(chain)
    }

    pub fn tip(&self) -> (Hash, u64) {
        self.storage.tip()
    }

    pub fn get_block(&self, hash: &Hash) -> Option<Block> {
        self.storage.get_block(hash)
    }

    pub fn get_header(&self, hash: &Hash) -> Option<BlockHeader> {
        self.storage.get_header(hash)
    }

    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        self.storage.get_block_by_height(height)
    }

    pub fn process_block(&mut self, block: Block) -> Result<bool, String> {
        let hash = block.hash();
        if self.storage.has_block(&hash) {
            return Ok(false);
        }
        let parent_hash = block.header.previous_hash;
        let parent_height = if parent_hash == [0u8; 32] {
            None
        } else {
            self.storage.height_of(&parent_hash)
        };
        if parent_hash != [0u8; 32] && parent_height.is_none() {
            self.orphan_blocks.insert(hash, block);
            return Ok(false);
        }
        let height = parent_height.map(|h| h + 1).unwrap_or(0);
        let difficulty = Consensus::compute_difficulty(&self.storage, height, &parent_hash, &self.config.genesis_difficulty);
        Consensus::validate_block(&block, &self.storage, &difficulty)?;
        self.storage.store_block(&block, height)
            .map_err(|e| format!("storage error: {}", e))?;
        self.try_adopt_orphans();
        self.try_reorg()?;
        if height % 1000 == 0 {
            let _ = self.storage.prune();
        }
        Ok(true)
    }

    pub fn build_block(
        &self,
        miner_address: Address,
        difficulty_vote: i8,
        payloads: Vec<Payload>,
    ) -> Result<Block, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.build_block_with_timestamp(miner_address, difficulty_vote, payloads, timestamp)
    }

    pub fn build_block_with_timestamp(
        &self,
        miner_address: Address,
        difficulty_vote: i8,
        payloads: Vec<Payload>,
        timestamp: u64,
    ) -> Result<Block, String> {
        let (tip_hash, _tip_height) = self.tip();
        let payloads: Vec<Payload> = payloads.into_iter()
            .take(MAX_PAYLOADS_PER_BLOCK)
            .collect();
        let merkle_root = Block::compute_merkle_root(&payloads);
        let header = BlockHeader {
            previous_hash: tip_hash,
            timestamp,
            miner_address,
            difficulty_vote,
            payload_count: payloads.len() as u32,
            merkle_root,
        };
        Ok(Block {
            header,
            nonce: 0,
            payloads,
        })
    }

    pub fn current_difficulty(&self) -> Difficulty {
        let (_, height) = self.tip();
        if height == 0 {
            self.config.genesis_difficulty.clone()
        } else {
            Consensus::difficulty_at_height(&self.storage, height, &self.config.genesis_difficulty)
        }
    }

    pub fn storage(&self) -> &BlockStorage {
        &self.storage
    }

    fn create_genesis(&mut self) -> Result<(), std::io::Error> {
        let header = BlockHeader {
            previous_hash: [0u8; 32],
            timestamp: self.config.genesis_timestamp,
            miner_address: [0u8; 32],
            difficulty_vote: 0,
            payload_count: 1,
            merkle_root: Block::compute_merkle_root(&[self.config.genesis_payload]),
        };
        let genesis = Block {
            header,
            nonce: 0,
            payloads: vec![self.config.genesis_payload],
        };
        self.storage.store_block(&genesis, 0)?;
        Ok(())
    }

    fn try_adopt_orphans(&mut self) {
        let mut adopted = Vec::new();
        for (hash, block) in &self.orphan_blocks {
            let parent = block.header.previous_hash;
            if self.storage.has_block(&parent) {
                adopted.push(*hash);
            }
        }
        for hash in adopted {
            if let Some(block) = self.orphan_blocks.remove(&hash) {
                let _ = self.process_block(block);
            }
        }
    }

    fn try_reorg(&mut self) -> Result<(), String> {
        let (tip_hash, _) = self.tip();
        let all_hashes = self.storage.all_hashes();
        let best = match Consensus::select_best_chain(&self.storage, &all_hashes, &self.config.genesis_difficulty) {
            Some(h) => h,
            None => return Ok(()),
        };
        if best == tip_hash {
            return Ok(());
        }
        let common = Consensus::find_common_ancestor(&self.storage, &tip_hash, &best)
            .unwrap_or([0u8; 32]);
        let old_branch = Consensus::get_ancestors(&self.storage, &tip_hash);
        let new_branch = Consensus::get_ancestors(&self.storage, &best);
        let mut old_to_apply: Vec<Hash> = old_branch.into_iter()
            .take_while(|h| *h != common)
            .collect();
        let mut new_to_apply: Vec<Hash> = new_branch.into_iter()
            .take_while(|h| *h != common)
            .collect();
        old_to_apply.reverse();
        new_to_apply.reverse();
        for hash in &old_to_apply {
            tracing::info!("reorg: rolling back {}", hex::encode(hash));
        }
        for hash in &new_to_apply {
            tracing::info!("reorg: applying {}", hex::encode(hash));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Payload, ChainConfig};
    use tempfile::TempDir;

    #[test]
    fn test_chain_creation() {
        let tmp = TempDir::new().unwrap();
        let chain = Blockchain::open(tmp.path(), ChainConfig::mainnet()).unwrap();
        let (tip, height) = chain.tip();
        assert_eq!(height, 0);
        let genesis = chain.get_block(&tip).unwrap();
        assert_eq!(genesis.header.previous_hash, [0u8; 32]);
    }

    fn easy_config() -> ChainConfig {
        ChainConfig {
            genesis_timestamp: 1_700_000_000,
            genesis_difficulty: Difficulty::new([0xFFu8; 32]),
            genesis_payload: Payload([0u8; 256]),
        }
    }

    #[test]
    #[ignore = "scrypt N=32768 is too slow in debug mode; run with --release"]
    fn test_process_valid_block() {
        let tmp = TempDir::new().unwrap();
        let mut chain = Blockchain::open(tmp.path(), easy_config()).unwrap();
        let (tip, _) = chain.tip();
        let mut block = chain.build_block([1u8; 32], 0, vec![]).unwrap();
        let difficulty = chain.current_difficulty();
        for nonce in 0..1_000_000 {
            block.nonce = nonce;
            if difficulty.satisfies(&block.pow_hash()) {
                break;
            }
        }
        let result = chain.process_block(block);
        assert!(result.is_ok());
        assert_eq!(chain.tip().1, 1);
    }

    #[test]
    fn test_process_duplicate_block() {
        let tmp = TempDir::new().unwrap();
        let mut chain = Blockchain::open(tmp.path(), ChainConfig::mainnet()).unwrap();
        let (tip, _) = chain.tip();
        let genesis = chain.get_block(&tip).unwrap();
        let result = chain.process_block(genesis.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }
}
