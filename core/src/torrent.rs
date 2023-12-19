use std::{
    collections::HashMap,
    net::SocketAddr,
    time::Instant,
    sync::Arc,
};
use rand::seq::SliceRandom;
use tokio::sync::{mpsc::{self, UnboundedReceiver}, RwLock};
use crate::{
    ctx::TorrentContext,
    p2p::{session::PeerSession, peer::PeerHandle},
    tracker::{Tracker, Event, AnnounceParams, TrackerError},
    metainfo::MetaInfo, 
    stats::Stats, 
    piece_selector::PieceSelector, fs::File,
};

// More aggressively search for peers when num < MIN_PEERS_PER_TORRENT
const MAX_PEERS_PER_TORRENT: usize = 100;
const MIN_PEERS_PER_TORRENT: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {

    #[error("Tracker Error: {0}")]
    TrackerError(#[from] TrackerError),
    
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

}

pub type Result<T> = std::result::Result<T, TorrentError>;

// Commands that can be sent to a torrent.
pub enum CommandToTorrent {
    // Sent by a peer when successfully connected.
    PeerConnected { address: SocketAddr, id: [u8; 20] },

}

#[derive(Debug)]
pub struct Torrent {

    // Context is a read-only state accessible by peers in threads.
    ctx: Arc<TorrentContext>,

    // Trackers are ordered by tier.
    trackers: Vec<Vec<Tracker>>,

    // Peers we have active sessions with.
    peers: HashMap<SocketAddr, PeerHandle>,

    // Peers we know about but don't have a session with.
    available: Vec<SocketAddr>,

    // Receiver for commands.
    cmd_rx: UnboundedReceiver<CommandToTorrent>,

    // Time when torrent started.
    start_time: Option<Instant>,

    // Statistics for upload/download etc.
    stats: Stats,

    // Files for the torrent.
    files: Vec<File>,

    // Address to listen for incoming connections on.
    listen_address: SocketAddr,

}

pub struct TorrentConfig {
    pub client_id: [u8; 20],
    pub listen_address: SocketAddr,
}

impl Torrent {

    pub fn new(metainfo: MetaInfo, config: TorrentConfig) -> Torrent {

        // If the "announce-list" key is present, the client will ignore the "announce" key and only use the URLs in "announce-list" (BEP-12).
        let trackers = if let Some(announce_list) = metainfo.announce_list.clone() {
            let mut trackers = Vec::new();
            for mut tier in announce_list {
                let mut tier_trackers = Vec::new();
                tier.shuffle(&mut rand::thread_rng());
                for url in tier {
                    tier_trackers.push(Tracker::new(url));
                }
                trackers.push(tier_trackers);
            }
            trackers
        // Otherwise we just use the announce key.
        } else {
            vec![vec![Tracker::new(metainfo.announce.clone())]]
        };

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        Torrent {
            ctx: Arc::new(
                TorrentContext {
                    info_hash: metainfo.info_hash,
                    client_id: config.client_id,
                    piece_selector: Arc::new(RwLock::new(PieceSelector::new(metainfo.num_pieces()))),
                    total_size: metainfo.total_size(),
                    num_pieces: metainfo.num_pieces(),
                    cmd_tx: cmd_tx.clone(),
                }
            ),
            trackers,
            peers: HashMap::new(),
            available: Vec::new(),
            cmd_rx,
            start_time: None,
            stats: Stats::default(),
            files: metainfo.files(),
            listen_address: config.listen_address,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("starting torrent");
        self.start_time = Some(Instant::now());
        // Announce start event to trackers.
        self.announce(Some(Event::Started), Instant::now()).await?;
        // Run until there is an error.
        // self.run().await?;
        Ok(())
    }

    // TODO: reorder trackers within tiers based on whether we can connect to trackers.
    #[tracing::instrument(skip(self, time), fields(num_peers = self.peers.len() + self.available.len()))]
    pub async fn announce(&mut self, event: Option<Event>, time: Instant) -> Result<()> {

        // Use trackers in order of tiers/priority.
        for tier in self.trackers.iter_mut() {
            for tracker in tier {

                let num_peers = self.peers.len() + self.available.len();
                // Number of peers we absolutely require.
                let num_peers_essential = if num_peers >= MIN_PEERS_PER_TORRENT || event == Some(Event::Stopped) {
                    None
                } else {
                    Some((MAX_PEERS_PER_TORRENT - num_peers).max(MIN_PEERS_PER_TORRENT))
                };

                // If event OR we need peers and we can announce OR we can have more peers and should announce, then announce.
                if event.is_some() || (num_peers_essential > Some(0) && tracker.can_announce(time)) || tracker.should_announce(time) {
                    
                    let params = AnnounceParams {
                        info_hash: self.ctx.info_hash,
                        peer_id: self.ctx.client_id,
                        port: DEFAULT_PORT,
                        uploaded: self.stats.uploaded,
                        downloaded: self.stats.downloaded,
                        left: self.ctx.total_size - self.stats.downloaded,
                        event,
                        num_want: num_peers_essential,
                        tracker_id: tracker.tracker_id.clone(),
                    };

                    let peers = tracker.send_announce(params).await?;
                    self.available.extend(peers.into_iter());
                    tracker.last_announce = Some(time);
                }

            }
        }

        tracing::trace!("new number of peers: {}", self.peers.len() + self.available.len());
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "torrent")]
    async fn run(&mut self) -> Result<()> {

        let listener = tokio::net::TcpListener::bind(&self.listen_address).await?;
        self.listen_address = listener.local_addr()?;
        tracing::info!("listening on {}", self.listen_address);
        self.connect_to_peers();

        loop { tokio::select! {

            new_peer_conn = listener.accept() => {
                let (stream, address) = match new_peer_conn {
                    Ok((stream, address)) => (stream, address),
                    Err(e) => {
                        tracing::warn!("inbound peer connection error: {}", e);
                        continue;
                    },
                };
                let (session, cmd_out) = PeerSession::new(address, self.ctx.clone());
                self.peers.insert(address, PeerHandle::start_session(session, cmd_out, Some(stream)));
            }

            Some(cmd) = self.cmd_rx.recv() => {
                match cmd {
                    CommandToTorrent::PeerConnected { address, id } => {

                        if let Some(peer) = self.peers.get_mut(&address) {
                            tracing::info!("peer {} connected", address);
                            peer.id = Some(id);
                        }
                    }
                }
            }
        }}

        #[allow(unreachable_code)]
        Ok(())
    }

    fn connect_to_peers(&mut self) {
        let count = self.available.len().min(MAX_PEERS_PER_TORRENT.saturating_sub(self.peers.len()));
        if count == 0 {
            tracing::info!("no peers to connect to");
            return;
        }

        tracing::info!("connecting to {} peers", count);
        for address in self.available.drain(0..count) {
            let (session, cmd) = PeerSession::new(address, self.ctx.clone());
            self.peers.insert(address, PeerHandle::start_session(session, cmd, None));
        }
    }
}
