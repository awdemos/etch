use etch::{
    chain::Blockchain,
    cli::{Cli, Commands},
    crypto,
    miner::Miner,
    p2p::{P2PNode, P2PEvent},
    types::{ChainConfig, Payload},
};
use clap::Parser;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::GenerateKey => {
            let keypair = crypto::generate_keypair();
            println!("address:  {}", hex::encode(keypair.public));
            println!("secret:   {}", hex::encode(keypair.secret));
            println!("WARNING: save the secret key. there is no recovery.");
        }
        Commands::SubmitPayload { payload, data_dir } => {
            let bytes = hex::decode(&payload)?;
            if bytes.len() != 256 {
                eprintln!("payload must be exactly 256 bytes (got {})", bytes.len());
                std::process::exit(1);
            }
            let mut payload_array = [0u8; 256];
            payload_array.copy_from_slice(&bytes);
            let payload_file = data_dir.join("payloads.txt");
            let hex_payload = hex::encode(payload_array);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&payload_file)?
                .write_all(format!("{}\n", hex_payload).as_bytes())?;
            println!("payload submitted to {}", payload_file.display());
        }
        Commands::Mine {
            secret,
            listen,
            peer,
            data_dir,
        } => {
            let secret_bytes = hex::decode(&secret)?;
            if secret_bytes.len() != 32 {
                eprintln!("secret must be 32 bytes");
                std::process::exit(1);
            }
            let mut secret_array = [0u8; 32];
            secret_array.copy_from_slice(&secret_bytes);
            let keypair = crypto::load_keypair(&secret_array)
                .expect("invalid secret key");
            run_node(data_dir, listen, peer, Some(keypair.public)).await?;
        }
        Commands::Node {
            listen,
            peer,
            data_dir,
        } => {
            run_node(data_dir, listen, peer, None).await?;
        }
    }
    Ok(())
}

async fn run_node(
    data_dir: PathBuf,
    listen: String,
    peers: Vec<String>,
    miner_address: Option<[u8; 32]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let chain_dir = data_dir.join("chain");
    let blockchain = Arc::new(Mutex::new(Blockchain::open(
        &chain_dir,
        ChainConfig::mainnet(),
    )?));
    let (block_tx, mut block_rx) = mpsc::channel(1024);
    let (payload_tx, payload_rx) = mpsc::channel(1024);
    let listen_addr: libp2p::Multiaddr = listen.parse()?;
    let bootstrap_peers: Vec<libp2p::Multiaddr> = peers
        .iter()
        .map(|p| p.parse())
        .collect::<Result<Vec<_>, _>>()?;
    let (p2p_node, mut p2p_events) = P2PNode::new(listen_addr, bootstrap_peers)?;
    let p2p_block_sender = p2p_node.block_sender();
    tokio::spawn(async move {
        let (event_tx, _event_rx) = mpsc::channel(1024);
        p2p_node.run(event_tx).await;
    });
    if let Some(address) = miner_address {
        let miner = Miner::new(
            blockchain.clone(),
            address,
            payload_rx,
            block_tx.clone(),
        );
        tokio::spawn(miner.run());
    }
    let payload_file = data_dir.join("payloads.txt");
    let payload_tx_clone = payload_tx.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(content) = std::fs::read_to_string(&payload_file) {
                let lines: Vec<_> = content.lines().collect();
                if !lines.is_empty() {
                    for line in lines {
                        if let Ok(bytes) = hex::decode(line.trim()) {
                            if bytes.len() == 256 {
                                let mut payload = [0u8; 256];
                                payload.copy_from_slice(&bytes);
                                let _ = payload_tx_clone.send(Payload(payload)).await;
                            }
                        }
                    }
                    let _ = std::fs::remove_file(&payload_file);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });
    let p2p_block_sender_clone = p2p_block_sender.clone();
    tokio::spawn(async move {
        while let Some(block) = block_rx.recv().await {
            let _ = p2p_block_sender_clone.send(block).await;
        }
    });
    loop {
        tokio::select! {
            Some(event) = p2p_events.recv() => {
                match event {
                    P2PEvent::BlockReceived(block) => {
                        let mut guard = blockchain.lock().await;
                        match guard.process_block(block) {
                            Ok(true) => {
                                tracing::info!("accepted block from p2p");
                            }
                            Ok(false) => {}
                            Err(e) => {
                                tracing::debug!("rejected block: {}", e);
                            }
                        }
                    }
                    P2PEvent::PeerConnected(peer_id) => {
                        tracing::info!("peer connected: {}", peer_id);
                    }
                    P2PEvent::PeerDisconnected(peer_id) => {
                        tracing::info!("peer disconnected: {}", peer_id);
                    }
                }
            }
        }
    }
}
