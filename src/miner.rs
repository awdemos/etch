use crate::types::{Block, Address, Payload, MAX_PAYLOADS_PER_BLOCK};
use crate::chain::Blockchain;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct Miner {
    blockchain: Arc<Mutex<Blockchain>>,
    miner_address: Address,
    payload_rx: mpsc::Receiver<Payload>,
    block_tx: mpsc::Sender<Block>,
}

impl Miner {
    pub fn new(
        blockchain: Arc<Mutex<Blockchain>>,
        miner_address: Address,
        payload_rx: mpsc::Receiver<Payload>,
        block_tx: mpsc::Sender<Block>,
    ) -> Self {
        Self {
            blockchain,
            miner_address,
            payload_rx,
            block_tx,
        }
    }

    pub async fn run(mut self) {
        loop {
            let mut payloads = Vec::new();
            while let Ok(payload) = self.payload_rx.try_recv() {
                payloads.push(payload);
                if payloads.len() >= MAX_PAYLOADS_PER_BLOCK {
                    break;
                }
            }
            let blockchain = self.blockchain.clone();
            let miner_address = self.miner_address;
            let block_tx = self.block_tx.clone();
            tokio::spawn(async move {
                let guard = blockchain.lock().await;
                let difficulty_vote = 0i8;
                let mut block = match guard.build_block(miner_address, difficulty_vote, payloads) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!("build block: {}", e);
                        return;
                    }
                };
                let difficulty = guard.current_difficulty();
                drop(guard);
                let found = tokio::task::spawn_blocking(move || {
                    for nonce in 0..u64::MAX {
                        block.nonce = nonce;
                        if difficulty.satisfies(&block.pow_hash()) {
                            return Some(block);
                        }
                        if nonce % 1_000_000 == 0 {
                            std::thread::yield_now();
                        }
                    }
                    None
                }).await;
                match found {
                    Ok(Some(block)) => {
                        let mut guard = blockchain.lock().await;
                        match guard.process_block(block.clone()) {
                            Ok(true) => {
                                tracing::info!("mined block {} with nonce {}", hex::encode(block.hash()), block.nonce);
                                let _ = block_tx.send(block).await;
                            }
                            Ok(false) => {
                                tracing::debug!("block already known");
                            }
                            Err(e) => {
                                tracing::error!("process mined block: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("exhausted nonce space");
                    }
                    Err(e) => {
                        tracing::error!("mining task: {}", e);
                    }
                }
            });
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
