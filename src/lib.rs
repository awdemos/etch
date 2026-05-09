//! # Etch
//!
//! A minimal Scrypt-PoW timestamping blockchain library.
//!
//! **What it does:** timestamp arbitrary 256-byte payloads into an immutable chain.
//!
//! **What it doesn't do:** smart contracts, VM, accounts, balances, UTXOs,
//! staking, governance, or anything else.
//!
//! ## Quick Start
//!
//! ```no_run
//! use etch::{Blockchain, ChainConfig, Payload, crypto::generate_keypair};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let keys = generate_keypair();
//! let config = ChainConfig::mainnet();
//! let mut chain = Blockchain::open(Path::new("./data"), config)?;
//!
//! let payload = Payload([0u8; 256]);
//! let block = chain.build_block(keys.public, 0, vec![payload])?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`types`] | Core data structures: [`Block`], [`BlockHeader`], [`Payload`], [`Difficulty`] |
//! | [`chain`] | Blockchain state: [`Blockchain`] manages the tip, validation, reorgs |
//! | [`storage`] | Flat-file persistence: [`BlockStorage`] for append-only block storage |
//! | [`consensus`] | Validation rules: [`Consensus::validate_block`] |
//! | [`crypto`] | Ed25519 keys, block signing, key generation |
//! | [`miner`] | Async mining loop: [`Miner`] mines blocks with Scrypt PoW |
//! | [`cli`] | CLI argument parsing (binary-only, not typically needed as a library) |
//! | [`p2p`] | libp2p networking (binary-only, not typically needed as a library) |

// Core types — re-exported at crate root for ergonomic use
pub use types::{
    Block, BlockHeader, ChainConfig, Difficulty, Hash, Address, Payload,
    MAX_PAYLOADS_PER_BLOCK, TARGET_BLOCK_TIME_SECS, BLOCKS_PER_YEAR,
    DIFFICULTY_ADJUSTMENT_PERIOD, DIFFICULTY_VOTE_WINDOW, BLOCK_REWARD,
    REWARD_DECIMALS, MAX_SUPPLY, SCRYPT_N, SCRYPT_R, SCRYPT_P, SCRYPT_LEN,
};

// Chain management
pub use chain::Blockchain;

// Storage layer
pub use storage::BlockStorage;

// Consensus validation
pub use consensus::Consensus;

// Miner
pub use miner::Miner;

// Modules
pub mod chain;
pub mod cli;
pub mod consensus;
pub mod crypto;
pub mod miner;
pub mod p2p;
pub mod storage;
pub mod types;
