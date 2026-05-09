use etch::{Blockchain, ChainConfig, Difficulty, Payload, crypto::generate_keypair};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = generate_keypair();
    let data_dir = Path::new("./example_data_basic");
    let _ = std::fs::remove_dir_all(data_dir);

    let mut config = ChainConfig::mainnet();
    config.genesis_difficulty = Difficulty::new([
        0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);

    let mut chain = Blockchain::open(data_dir, config)?;
    println!("Genesis block hash: {}", hex::encode(chain.tip().0));

    let payload = Payload([42u8; 256]);
    let mut block = chain.build_block(keys.public, 0, vec![payload])?;

    let difficulty = chain.current_difficulty();
    let mut nonce = 0u64;
    loop {
        block.nonce = nonce;
        if difficulty.satisfies(&block.pow_hash()) {
            break;
        }
        nonce += 1;
        if nonce % 1_000_000 == 0 {
            println!("Tried {} nonces...", nonce);
        }
    }

    println!("Found nonce: {}", nonce);
    println!("Block hash:  {}", hex::encode(block.hash()));
    println!("PoW hash:    {}", hex::encode(block.pow_hash()));

    chain.process_block(block)?;
    println!("Tip height: {}", chain.tip().1);

    let _ = std::fs::remove_dir_all(data_dir);
    Ok(())
}
