# Etch

> A minimal Scrypt-PoW timestamping blockchain in Rust. No smart contracts, no VM, no accounts, no balances, no UTXOs, no staking, no governance.

Etch carves immutable timestamps into a distributed chain. Each block contains up to 1024 payloads of exactly 256 bytes, proven with Scrypt proof-of-work, signed by an Ed25519 miner key, and gossipped across a libp2p network.

## What It Does

- **Timestamp 256-byte payloads** with Scrypt proof-of-work (N=32768, r=8, p=1, ~2 min target)
- **Gossip blocks** over libp2p to connected peers via gossipsub
- **Adjust difficulty** once per year by simple majority vote of the last 1000 blocks
- **Store blocks** in flat binary files with automatic pruning (last 1000 blocks kept full)
- **Mine blocks** with CPU-friendly Scrypt on commodity hardware
- **Fork resolution** by cumulative work with orphan adoption

## What It Does Not Do

- No Turing-complete VM, no smart contracts
- No account balances, no transfers, no UTXOs
- No staking, no slashing, no governance tokens
- No dynamic peer discovery (static bootstrap lists only)
- No mempool (payloads read from a file, one per line)
- No wallets, no recovery, no key encryption

## Quick Start

```bash
# Build the project
cargo build --release

# Generate a miner keypair (save the secret!)
cargo run --release -- generate-key
# address:  a1b2c3...
# secret:   d4e5f6...

# Start a node
cargo run --release -- node --data-dir ~/.etch --peer /ip4/1.2.3.4/tcp/9090/p2p/12D3...

# Submit a payload (must be exactly 256 bytes, hex-encoded)
cargo run --release -- submit-payload --payload $(python3 -c "print('00'*256)")

# Mine blocks locally
cargo run --release -- mine --data-dir ~/.etch --secret <your-secret-hex>
```

## Architecture

| Module | Responsibility |
|--------|---------------|
| `types` | Block, Header, Payload, Difficulty, ChainConfig, Merkle root |
| `crypto` | Ed25519 key generation, signing, and verification |
| `storage` | Flat-file append-only block storage with index and pruning |
| `consensus` | Block validation, difficulty adjustment, cumulative work, reorg |
| `chain` | Blockchain state management, tip tracking, orphan adoption |
| `p2p` | libp2p gossipsub for block propagation, identify, ping |
| `miner` | Scrypt PoW mining loop with nonce iteration |
| `cli` | Clap-based command-line interface |

## Consensus Rules

- **Genesis block** is hardcoded and exempt from PoW validation
- **Block time target**: 2 minutes (720 blocks/day)
- **Difficulty vote**: `i8` in block header (`-1` = decrease, `0` = keep, `+1` = increase)
- **Annual adjustment**: simple majority of last 1000 blocks (>500 agreeing votes)
- **Chain selection**: most cumulative Scrypt work
- **Max payloads**: 1024 per block
- **Payload size**: exactly 256 bytes each
- **Signature**: Ed25519 over block hash by miner

## Block Structure

```rust
Block {
    header: BlockHeader {
        previous_hash: [u8; 32],    // SHA-256 of parent
        merkle_root: [u8; 32],      // SHA-256 of payload Merkle tree
        timestamp: u64,             // Unix seconds
        miner_address: [u8; 32],    // Ed25519 public key
        difficulty_vote: i8,        // -1, 0, or +1
    },
    payload_list: Vec<[u8; 256]>,   // Up to 1024 payloads
    nonce: u64,                     // Scrypt PoW nonce
    signature: [u8; 64],            // Ed25519 signature
}
```

## Storage Format

Flat binary files in the data directory:
- `chain.bin` — append-only raw blocks
- `index.bin` — height → file offset mapping
- `meta.bin` — chain tip, height, difficulty state

Older blocks beyond the last 1000 are pruned to header-only to bound disk usage.

## Testing

```bash
# Unit tests (fast)
cargo test --lib

# Integration test: 3 nodes, 100 blocks, consensus verification
# This takes ~60 seconds in release mode due to Scrypt
cargo test --release --test integration -- --ignored
```

## Protocol Parameters

| Parameter | Value |
|-----------|-------|
| Scrypt N | 32,768 |
| Scrypt r | 8 |
| Scrypt p | 1 |
| Target block time | 120 seconds |
| Max payloads/block | 1,024 |
| Payload size | 256 bytes |
| Key algorithm | Ed25519 |
| Serialization | Postcard |
| P2P transport | libp2p (TCP + Noise + Yamux + Gossipsub) |
| Difficulty adjustment | Annual, miner majority vote |

## License

MIT
