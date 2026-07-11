# PROJECT KNOWLEDGE BASE

**Generated:** 2026-05-24
**Commit:** ab8345d
**Branch:** main

## OVERVIEW

Etch: minimal Scrypt-PoW timestamping blockchain in Rust. Dual crate (lib + bin). No smart contracts, VM, accounts, balances, UTXOs, staking, or governance. Just immutable 256-byte payloads in blocks, proven with Scrypt, signed by Ed25519, gossiped over libp2p.

## STRUCTURE

```
etch/
├── Cargo.toml          # Dual crate manifest (lib + bin, both named "etch")
├── src/
│   ├── lib.rs          # Crate root: re-exports + pub mod declarations
│   ├── main.rs         # Binary entry: tokio async CLI dispatcher
│   ├── types.rs        # Block, Header, Payload, Difficulty, constants
│   ├── chain.rs        # Blockchain state: open, tip, process_block, reorg
│   ├── storage.rs      # Flat-file persistence: chain.bin, index.bin, meta.bin
│   ├── consensus.rs    # Validation, difficulty adjustment, cumulative work
│   ├── crypto.rs       # Ed25519 keygen, signing, verification
│   ├── miner.rs        # Async Scrypt PoW mining loop
│   ├── p2p.rs          # libp2p gossipsub block propagation
│   └── cli.rs          # Clap argument parsing
├── examples/
│   ├── basic_chain.rs
│   ├── custom_payload.rs
│   └── verify_block.rs
└── tests/
    └── integration.rs  # debug_timestamp + three_nodes_mine_100_blocks
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add new block field | `src/types.rs` | Also update postcard serde + consensus checks |
| Change PoW params | `src/types.rs` (constants) | `SCRYPT_N`, `SCRYPT_R`, `SCRYPT_P` |
| Change difficulty rules | `src/consensus.rs` | `compute_difficulty`, `adjust_target` |
| Change storage format | `src/storage.rs` | `PRUNE_DEPTH`, postcard serialization |
| Change P2P protocol | `src/p2p.rs` | `BLOCKS_TOPIC`, gossipsub config |
| Add CLI command | `src/cli.rs` + `src/main.rs` match arm |
| Mine a block (library) | `src/miner.rs` | `Miner::run()` collects payloads + spawn_blocking PoW |
| Integration test | `tests/integration.rs` | Requires `--release` (Scrypt too slow in debug) |

## CODE MAP

| Symbol | Type | File | Role |
|--------|------|------|------|
| `Blockchain` | struct | `chain.rs` | Chain state: tip, process, build, reorg |
| `BlockStorage` | struct | `storage.rs` | Flat-file IO: store/get/prune |
| `Consensus` | struct (namespace) | `consensus.rs` | Static validation + difficulty logic |
| `Miner` | struct | `miner.rs` | Async mining worker |
| `P2PNode` | struct | `p2p.rs` | libp2p swarm wrapper |
| `Block` | struct | `types.rs` | Header + nonce + payloads |
| `BlockHeader` | struct | `types.rs` | prev_hash, timestamp, miner, vote, count, merkle |
| `Difficulty` | struct | `types.rs` | Target hash threshold |
| `Payload` | struct | `types.rs` | Fixed 256-byte array wrapper |
| `Cli` / `Commands` | enums | `cli.rs` | Clap CLI definition |
| `generate_keypair` | fn | `crypto.rs` | Ed25519 key generation |
| `sign_block` | fn | `crypto.rs` | **UNUSED** — signs block hash |
| `verify_block_signature` | fn | `crypto.rs` | **UNUSED** — verifies block sig |

## CONVENTIONS

- **Dual crate**: `lib.rs` re-exports core types at crate root; `main.rs` is the CLI binary. Both named `etch` in Cargo.toml.
- **All modules are `pub mod`**: No private module boundaries. `cli` and `p2p` are documented as "binary-only" but remain fully public.
- **Postcard for serialization**: All block/network serialization uses `postcard` (compact binary), not JSON.
- **Flat-file storage**: Append-only `chain.bin` with `index.bin` (HashMap) and `meta.bin` (tip state). No database.
- **No unified error type**: Uses `Result<..., String>` and `Result<..., std::io::Error>` directly. `thiserror` dependency is present but unused.
- **Test helpers are public**: `build_block_with_timestamp` is `pub` for integration test use.

## ANTI-PATTERNS (THIS PROJECT)

1. **P2P event channel is disconnected** (`main.rs:92`, `p2p.rs:95`). `p2p_events.recv()` in the main loop never resolves because `P2PNode::new()` drops the event sender, and the spawned task creates a separate channel whose receiver is also dropped. P2P events are lost.
2. **`try_reorg()` is a no-op** (`chain.rs:160-189`). Computes rollback/apply lists, logs them, but never actually updates storage or chain tip.
3. **Block signatures are unimplemented**. `crypto.rs` defines `sign_block`/`verify_block_signature`, but `Block` has no `signature` field and `Consensus::validate_block` never checks signatures.
4. **Unused dependencies**: `thiserror`, `chrono`, `serde-big-array` are in Cargo.toml but never referenced in source.
5. **Scrypt PoW logic in `types.rs`**. `Block::pow_hash()` calls `scrypt::scrypt()`. PoW belongs in `consensus.rs` or `miner.rs`.
6. **README block structure is wrong**. Documents `signature: [u8; 64]` and `payload_list` fields that do not exist in code.
7. **O(n) height lookup**. `BlockStorage::get_block_by_height` does a linear scan over the index HashMap instead of using a Vec/BTreeMap.
8. **`.expect()` / `.unwrap()` in production paths**. `Block::hash()`, `Block::pow_hash()`, storage serialization, and `main.rs` CLI parsing all panic on errors instead of returning `Result`.
9. **`block_work()` truncates target**. `consensus.rs` converts only the first 16 bytes of the 32-byte difficulty target to `u128`, breaking cumulative work accuracy.
10. **`DefaultHasher` for P2P message IDs**. `p2p.rs` uses `std::collections::hash_map::DefaultHasher` for gossipsub message IDs — not cryptographically secure and unstable across Rust versions.

## COMMANDS

```bash
# Build (release strongly recommended — Scrypt is slow in debug)
cargo build --release

# Run unit tests (fast)
cargo test --lib

# Run ignored integration test (slow — Scrypt N=32768)
cargo test --release --test integration -- --ignored

# Generate miner keypair
cargo run --release -- generate-key

# Start a node
cargo run --release -- node --data-dir ~/.etch --peer /ip4/.../tcp/9090/p2p/...

# Submit a payload (exactly 256 bytes, hex-encoded)
cargo run --release -- submit-payload --payload $(python3 -c "print('00'*256)")

# Mine locally
cargo run --release -- mine --data-dir ~/.etch --secret <hex-secret>
```

## NOTES

- **Scrypt N=32768** means mining and some tests are ~60s in release, unusable in debug.
- **Difficulty vote is hardcoded to 0** in `miner.rs`. The voting mechanism exists in consensus but miners never participate.
- **No dynamic peer discovery**: Static bootstrap peer lists only.
- **No mempool**: Payloads are read from a flat text file (`payloads.txt`), one hex-encoded 256-byte payload per line.
- **No wallets, key encryption, or recovery**: Keys are raw 32-byte hex. Save the secret or lose it.
- **Genesis block** is hardcoded in `ChainConfig::mainnet()` and auto-created on first `Blockchain::open()`.
- **Pruning**: Blocks older than 1000 from tip are stripped to header-only to bound disk usage.

## Deployment

No Dagger module or recognized deployment configuration was found.

General redeploy process:

1. Commit and push changes to the default branch.
2. Trigger the relevant CI/CD pipeline or run the documented deploy command.
3. If the project is served via GitHub Pages, the site redeploys automatically after the push.
