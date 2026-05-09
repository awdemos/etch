use etch::{Blockchain, ChainConfig, Difficulty, Payload, Consensus, crypto::generate_keypair};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = generate_keypair();
    let data_dir = Path::new("./example_data_verify");
    let _ = std::fs::remove_dir_all(data_dir);

    let mut config = ChainConfig::mainnet();
    config.genesis_difficulty = Difficulty::new([
        0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);

    let mut chain = Blockchain::open(data_dir, config.clone())?;

    let payload = Payload([0u8; 256]);
    let mut block = chain.build_block(keys.public, 0, vec![payload])?;

    let difficulty = chain.current_difficulty();
    let mut nonce = 0u64;
    loop {
        block.nonce = nonce;
        if difficulty.satisfies(&block.pow_hash()) {
            break;
        }
        nonce += 1;
    }

    chain.process_block(block.clone())?;
    println!("Local chain accepted block at height {}", chain.tip().1);

    let storage = chain.storage();
    let current_difficulty = chain.current_difficulty();
    match Consensus::validate_block(&block, storage, &current_difficulty) {
        Ok(()) => println!("Consensus validation: PASSED"),
        Err(e) => println!("Consensus validation: FAILED - {}", e),
    }

    let mut bad_block = block.clone();
    bad_block.nonce = 999_999_999;
    match Consensus::validate_block(&bad_block, storage, &current_difficulty) {
        Ok(()) => println!("Bad block validation: PASSED (unexpected!)"),
        Err(e) => println!("Bad block correctly rejected: {}", e),
    }

    println!("Block hash:     {}", hex::encode(block.hash()));
    println!("Block merkle:   {}", hex::encode(block.header.merkle_root));
    println!("Block payload count: {}", block.header.payload_count);

    let _ = std::fs::remove_dir_all(data_dir);
    Ok(())
}
