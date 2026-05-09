use etch::chain::Blockchain;
use etch::types::{ChainConfig, Difficulty, Payload};
use etch::consensus::Consensus;
use tempfile::TempDir;

fn easy_config() -> ChainConfig {
    ChainConfig {
        genesis_timestamp: 1_700_000_000,
        genesis_difficulty: Difficulty::new([0xFFu8; 32]),
        genesis_payload: Payload([0u8; 256]),
    }
}

#[test]
fn debug_timestamp() {
    let tmp = TempDir::new().unwrap();
    let mut chain = Blockchain::open(tmp.path(), easy_config()).unwrap();
    let (tip, height) = chain.tip();
    println!("genesis hash: {:02x?}", &tip[..4]);
    println!("genesis height: {}", height);
    
    let genesis = chain.get_block(&tip).unwrap();
    println!("genesis timestamp: {}", genesis.header.timestamp);
    
    let block = chain.build_block([1u8; 32], 0, vec![]).unwrap();
    println!("new block timestamp: {}", block.header.timestamp);
    println!("new block previous_hash: {:02x?}", &block.header.previous_hash[..4]);
    
    let result = chain.process_block(block);
    println!("process result: {:?}", result);
}

#[test]
#[ignore = "slow: run with cargo test --release --test integration"]
fn three_nodes_mine_100_blocks() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();
    let tmp3 = TempDir::new().unwrap();

    let mut node1 = Blockchain::open(tmp1.path(), easy_config()).unwrap();
    let mut node2 = Blockchain::open(tmp2.path(), easy_config()).unwrap();
    let mut node3 = Blockchain::open(tmp3.path(), easy_config()).unwrap();

    let miner = [1u8; 32];

    for height in 1..=100 {
        let timestamp = easy_config().genesis_timestamp + height;
        let mut block = node1.build_block_with_timestamp([1u8; 32], 0, vec![], timestamp).unwrap();
        let difficulty = node1.current_difficulty();
        block.nonce = 0;
        assert!(difficulty.satisfies(&block.pow_hash()), "nonce=0 should satisfy easy target at height {}", height);
        
        node1.process_block(block.clone()).unwrap();
        node2.process_block(block.clone()).unwrap();
        node3.process_block(block.clone()).unwrap();
        
        if height % 10 == 0 {
            let tip1 = node1.tip();
            let tip2 = node2.tip();
            let tip3 = node3.tip();
            assert_eq!(tip1, tip2, "nodes 1 and 2 diverged at height {}", height);
            assert_eq!(tip2, tip3, "nodes 2 and 3 diverged at height {}", height);
            assert_eq!(tip1.1, height, "expected height {}, got {}", height, tip1.1);
        }
    }

    let (tip1, height1) = node1.tip();
    let (tip2, height2) = node2.tip();
    let (tip3, height3) = node3.tip();
    
    assert_eq!(height1, 100);
    assert_eq!(tip1, tip2);
    assert_eq!(tip2, tip3);
    
    let work = Consensus::cumulative_work(node1.storage(), &tip1, &node1.current_difficulty());
    assert!(work > 100);
}
