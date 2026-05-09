use etch::{Blockchain, ChainConfig, Difficulty, Payload, crypto::generate_keypair};
use std::path::Path;

fn string_to_payload(s: &str) -> Payload {
    let bytes = s.as_bytes();
    let mut buf = [0u8; 256];
    let len = bytes.len().min(256);
    buf[..len].copy_from_slice(&bytes[..len]);
    Payload(buf)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keys = generate_keypair();
    let data_dir = Path::new("./example_data_payloads");
    let _ = std::fs::remove_dir_all(data_dir);

    let mut config = ChainConfig::mainnet();
    config.genesis_difficulty = Difficulty::new([
        0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);

    let mut chain = Blockchain::open(data_dir, config)?;

    let payloads = vec![
        string_to_payload("Hello, Etch blockchain!"),
        string_to_payload("This is a custom payload."),
        string_to_payload("Each payload is exactly 256 bytes."),
    ];

    let mut block = chain.build_block(keys.public, 0, payloads)?;

    let difficulty = chain.current_difficulty();
    let mut nonce = 0u64;
    loop {
        block.nonce = nonce;
        if difficulty.satisfies(&block.pow_hash()) {
            break;
        }
        nonce += 1;
    }

    println!("Mined block with {} payloads", block.payloads.len());
    println!("Merkle root: {}", hex::encode(block.header.merkle_root));
    println!("Nonce: {}", nonce);

    for (i, payload) in block.payloads.iter().enumerate() {
        let text = String::from_utf8_lossy(&payload.0);
        println!("Payload {}: {}", i, text.trim_end_matches('\0'));
    }

    chain.process_block(block)?;
    println!("Chain tip height: {}", chain.tip().1);

    let _ = std::fs::remove_dir_all(data_dir);
    Ok(())
}
