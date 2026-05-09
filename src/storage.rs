use crate::types::{Block, BlockHeader, Hash};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const PRUNE_DEPTH: u64 = 1_000;

pub struct BlockStorage {
    data_dir: PathBuf,
    index: HashMap<Hash, (u64, u64)>,
    tip_hash: Hash,
    tip_height: u64,
}

impl BlockStorage {
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        let mut storage = Self {
            data_dir: data_dir.to_path_buf(),
            index: HashMap::new(),
            tip_hash: [0u8; 32],
            tip_height: 0,
        };
        storage.load_or_rebuild()?;
        Ok(storage)
    }

    pub fn tip(&self) -> (Hash, u64) {
        (self.tip_hash, self.tip_height)
    }

    pub fn get_block(&self, hash: &Hash) -> Option<Block> {
        let &(height, offset) = self.index.get(hash)?;
        self.read_block_at(offset, height).ok()
    }

    pub fn get_header(&self, hash: &Hash) -> Option<BlockHeader> {
        self.get_block(hash).map(|b| b.header)
    }

    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let hash = self.index.iter().find(|(_, (h, _))| *h == height)?.0;
        self.get_block(hash)
    }

    pub fn store_block(&mut self, block: &Block, height: u64) -> io::Result<()> {
        let hash = block.hash();
        let offset = self.append_block(block)?;
        self.index.insert(hash, (height, offset));
        self.tip_hash = hash;
        self.tip_height = height;
        self.save_meta()?;
        self.save_index()?;
        Ok(())
    }

    pub fn has_block(&self, hash: &Hash) -> bool {
        self.index.contains_key(hash)
    }

    pub fn height_of(&self, hash: &Hash) -> Option<u64> {
        self.index.get(hash).map(|(h, _)| *h)
    }

    pub fn all_hashes(&self) -> Vec<Hash> {
        self.index.keys().copied().collect()
    }

    fn chain_path(&self) -> PathBuf {
        self.data_dir.join("chain.bin")
    }

    fn index_path(&self) -> PathBuf {
        self.data_dir.join("index.bin")
    }

    fn meta_path(&self) -> PathBuf {
        self.data_dir.join("meta.bin")
    }

    fn load_or_rebuild(&mut self) -> io::Result<()> {
        if self.meta_path().exists() {
            self.load_meta()?;
        }
        if self.index_path().exists() {
            self.load_index()?;
        } else if self.chain_path().exists() {
            self.rebuild_index()?;
        }
        Ok(())
    }

    fn load_meta(&mut self) -> io::Result<()> {
        let bytes = fs::read(self.meta_path())?;
        if bytes.len() >= 40 {
            self.tip_hash.copy_from_slice(&bytes[0..32]);
            self.tip_height = u64::from_le_bytes([
                bytes[32], bytes[33], bytes[34], bytes[35],
                bytes[36], bytes[37], bytes[38], bytes[39],
            ]);
        }
        Ok(())
    }

    fn save_meta(&self) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(40);
        bytes.extend_from_slice(&self.tip_hash);
        bytes.extend_from_slice(&self.tip_height.to_le_bytes());
        fs::write(self.meta_path(), bytes)?;
        Ok(())
    }

    fn load_index(&mut self) -> io::Result<()> {
        let bytes = fs::read(self.index_path())?;
        if let Ok(index) = postcard::from_bytes(&bytes) {
            self.index = index;
        }
        Ok(())
    }

    fn save_index(&self) -> io::Result<()> {
        let bytes = postcard::to_stdvec(&self.index).expect("serialize index");
        fs::write(self.index_path(), bytes)?;
        Ok(())
    }

    fn rebuild_index(&mut self) -> io::Result<()> {
        let _file = File::open(self.chain_path())?;
        let mut offset = 0u64;
        let mut height = 0u64;
        loop {
            match self.read_block_at(offset, height) {
                Ok(block) => {
                    let hash = block.hash();
                    self.index.insert(hash, (height, offset));
                    self.tip_hash = hash;
                    self.tip_height = height;
                    let block_size = self.serialized_size(&block)?;
                    offset += 4 + block_size as u64;
                    height += 1;
                }
                Err(_) => break,
            }
        }
        self.save_meta()?;
        self.save_index()?;
        Ok(())
    }

    fn read_block_at(&self, offset: u64, expected_height: u64) -> io::Result<Block> {
        let mut file = File::open(self.chain_path())?;
        file.seek(SeekFrom::Start(offset))?;
        let mut len_bytes = [0u8; 4];
        file.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        let mut block_bytes = vec![0u8; len];
        file.read_exact(&mut block_bytes)?;
        let block: Block = postcard::from_bytes(&block_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Some((h, _)) = self.index.get(&block.hash()) {
            if *h != expected_height {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "height mismatch",
                ));
            }
        }
        Ok(block)
    }

    fn append_block(&self, block: &Block) -> io::Result<u64> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.chain_path())?;
        let offset = file.seek(SeekFrom::End(0))?;
        let bytes = postcard::to_stdvec(block).expect("serialize block");
        let len = bytes.len() as u32;
        file.write_all(&len.to_le_bytes())?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(offset)
    }

    fn serialized_size(&self, block: &Block) -> io::Result<usize> {
        Ok(postcard::to_stdvec(block).expect("serialize").len())
    }

    pub fn prune(&mut self) -> io::Result<()> {
        let prune_before = self.tip_height.saturating_sub(PRUNE_DEPTH);
        if prune_before == 0 {
            return Ok(());
        }
        let chain_path = self.chain_path();
        let temp_path = self.data_dir.join("chain.bin.tmp");
        {
            let mut new_file = File::create(&temp_path)?;
            let old_file = File::open(&chain_path)?;
            let mut reader = io::BufReader::new(old_file);
            for height in 0..=self.tip_height {
                let mut len_bytes = [0u8; 4];
                if reader.read_exact(&mut len_bytes).is_err() {
                    break;
                }
                let len = u32::from_le_bytes(len_bytes) as usize;
                let mut block_bytes = vec![0u8; len];
                reader.read_exact(&mut block_bytes)?;
                let offset_in_new = new_file.seek(SeekFrom::Current(0))?;
                if height < prune_before {
                    let block: Block = postcard::from_bytes(&block_bytes)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    let header_only = Block {
                        header: block.header,
                        nonce: block.nonce,
                        payloads: vec![],
                    };
                    let pruned_bytes = postcard::to_stdvec(&header_only)
                        .expect("serialize");
                    let pruned_len = pruned_bytes.len() as u32;
                    new_file.write_all(&pruned_len.to_le_bytes())?;
                    new_file.write_all(&pruned_bytes)?;
                } else {
                    new_file.write_all(&len_bytes)?;
                    new_file.write_all(&block_bytes)?;
                }
                if let Some(hash) = self.index.iter().find(|(_, (h, _))| *h == height).map(|(k, _)| *k) {
                    self.index.insert(hash, (height, offset_in_new));
                }
            }
            new_file.sync_all()?;
        }
        fs::rename(&temp_path, &chain_path)?;
        self.save_index()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Block, BlockHeader, Payload};
    use tempfile::TempDir;

    fn make_block(height: u64) -> Block {
        Block {
            header: BlockHeader {
                previous_hash: [height as u8; 32],
                timestamp: 1000 + height,
                miner_address: [0u8; 32],
                difficulty_vote: 0,
                payload_count: 1,
                merkle_root: [0u8; 32],
            },
            nonce: height,
            payloads: vec![Payload([height as u8; 256])],
        }
    }

    #[test]
    fn test_store_and_retrieve() {
        let tmp = TempDir::new().unwrap();
        let mut storage = BlockStorage::open(tmp.path()).unwrap();
        let block = make_block(0);
        let hash = block.hash();
        storage.store_block(&block, 0).unwrap();
        let retrieved = storage.get_block(&hash).unwrap();
        assert_eq!(retrieved.hash(), hash);
    }

    #[test]
    fn test_height_lookup() {
        let tmp = TempDir::new().unwrap();
        let mut storage = BlockStorage::open(tmp.path()).unwrap();
        for i in 0..5 {
            let block = make_block(i);
            storage.store_block(&block, i).unwrap();
        }
        let block = storage.get_block_by_height(2).unwrap();
        assert_eq!(block.nonce, 2);
    }

    #[test]
    fn test_prune() {
        let tmp = TempDir::new().unwrap();
        let mut storage = BlockStorage::open(tmp.path()).unwrap();
        for i in 0..1200 {
            let block = make_block(i);
            storage.store_block(&block, i).unwrap();
        }
        let tip = storage.tip().1;
        assert_eq!(tip, 1199);
        storage.prune().unwrap();
        let block0 = storage.get_block_by_height(0).unwrap();
        assert!(block0.payloads.is_empty());
        let block1199 = storage.get_block_by_height(1199).unwrap();
        assert!(!block1199.payloads.is_empty());
    }
}
