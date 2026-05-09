use crate::types::Block;
use futures::StreamExt;
use libp2p::{
    core, gossipsub, identify, identity, noise, ping, swarm::NetworkBehaviour,
    swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash as StdHash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;

const BLOCKS_TOPIC: &str = "etch-blocks";

#[derive(NetworkBehaviour)]
struct EtchBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct P2PNode {
    swarm: Swarm<EtchBehaviour>,
    block_tx: mpsc::Sender<Block>,
    block_rx: mpsc::Receiver<Block>,
    peers: HashSet<PeerId>,
}

#[derive(Debug, Clone)]
pub enum P2PEvent {
    BlockReceived(Block),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

impl P2PNode {
    pub fn new(
        listen_addr: Multiaddr,
        bootstrap_peers: Vec<Multiaddr>,
    ) -> Result<(Self, mpsc::Receiver<P2PEvent>), Box<dyn std::error::Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        tracing::info!("local peer id: {}", local_peer_id);

        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|e| format!("gossipsub config error: {}", e))?;

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        let topic = gossipsub::IdentTopic::new(BLOCKS_TOPIC);
        gossipsub.subscribe(&topic)?;

        let identify = identify::Behaviour::new(identify::Config::new(
            "/etch/0.1.0".to_string(),
            local_key.public(),
        ));

        let ping = ping::Behaviour::new(ping::Config::new());

        let behaviour = EtchBehaviour {
            gossipsub,
            identify,
            ping,
        };

        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default());
        let noise_config = noise::Config::new(&local_key)?;
        let transport = tcp_transport
            .upgrade(core::upgrade::Version::V1)
            .authenticate(noise_config)
            .multiplex(yamux::Config::default())
            .boxed();

        let mut swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            libp2p::swarm::Config::with_tokio_executor(),
        );

        let (block_tx, block_rx) = mpsc::channel(1024);
        let (_event_tx, event_rx) = mpsc::channel(1024);

        swarm.listen_on(listen_addr)?;
        for addr in bootstrap_peers {
            swarm.dial(addr)?;
        }

        let node = Self {
            swarm,
            block_tx,
            block_rx,
            peers: HashSet::new(),
        };

        Ok((node, event_rx))
    }

    pub async fn run(mut self, mut event_tx: mpsc::Sender<P2PEvent>) {
        let topic = gossipsub::IdentTopic::new(BLOCKS_TOPIC);
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event, &mut event_tx).await;
                }
                Some(block) = self.block_rx.recv() => {
                    let bytes = match postcard::to_stdvec(&block) {
                        Ok(b) => b,
                        Err(e) => {
                            tracing::error!("serialize block: {}", e);
                            continue;
                        }
                    };
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), bytes) {
                        tracing::debug!("publish error: {}", e);
                    }
                }
            }
        }
    }

    pub fn block_sender(&self) -> mpsc::Sender<Block> {
        self.block_tx.clone()
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<EtchBehaviourEvent>,
        event_tx: &mut mpsc::Sender<P2PEvent>,
    ) {
        match event {
            SwarmEvent::Behaviour(EtchBehaviourEvent::Gossipsub(
                gossipsub::Event::Message { propagation_source, message, .. },
            )) => {
                match postcard::from_bytes::<Block>(&message.data) {
                    Ok(block) => {
                        let _ = event_tx.send(P2PEvent::BlockReceived(block)).await;
                    }
                    Err(e) => {
                        tracing::debug!("invalid block from {}: {}", propagation_source, e);
                    }
                }
            }
            SwarmEvent::Behaviour(EtchBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                tracing::debug!("identified peer {}: {:?}", peer_id, info);
            }
            SwarmEvent::Behaviour(EtchBehaviourEvent::Ping(ping::Event {
                peer,
                result: Ok(_),
                ..
            })) => {
                tracing::trace!("ping from {}", peer);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.peers.insert(peer_id);
                let _ = event_tx.send(P2PEvent::PeerConnected(peer_id)).await;
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                self.peers.remove(&peer_id);
                let _ = event_tx.send(P2PEvent::PeerDisconnected(peer_id)).await;
            }
            _ => {}
        }
    }
}
