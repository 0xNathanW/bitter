use std::{
    collections::HashMap,
    net::SocketAddr,
    time::Instant,
    sync::Arc,
};
use tokio::sync::mpsc;
use crate::{
    p2p::{PeerHandle, PeerSession, PeerCommand},
    tracker::{Tracker, Event, AnnounceParams, TrackerError},
    metainfo::MetaInfo, 
    picker::Picker,
    store::StoreInfo, 
    fs, 
};

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {

    #[error("tracker error: {0}")]
    TrackerError(#[from] TrackerError),
    
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("channel error: {0}")]
    Channel(String),
}

impl<T> From<mpsc::error::SendError<T>> for TorrentError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        TorrentError::Channel(e.to_string())       
    }
}

// Type aliases.
pub type Result<T> = std::result::Result<T, TorrentError>;
pub type TorrentTx = mpsc::UnboundedSender<CommandToTorrent>;
pub type TorrentRx = mpsc::UnboundedReceiver<CommandToTorrent>;

// Commands that can be sent to a torrent from other tasks.
pub enum CommandToTorrent {

    // Sent by peer task when peer successfully connects.
    PeerConnected { address: SocketAddr, id: [u8; 20] },

    // Sent by disk task when piece written.
    PieceWritten { idx: usize, valid: bool },
    
    // Sent by itself to shutdown.
    Shutdown,

}

#[derive(Debug)]
pub struct Torrent {

    // Context is a read-only state accessible by peers in threads.
    ctx: Arc<TorrentContext>,

    // Peers we have active sessions with.
    peers: HashMap<SocketAddr, PeerHandle>,

    // Trackers are ordered by tier.
    trackers: Vec<Vec<Tracker>>,

    // Peers we know about but don't have a session with.
    available: Vec<SocketAddr>,

    // Receiver for commands.
    torrent_rx: TorrentRx,

    // Sender for commands, used on shutdown.
    torrent_tx: TorrentTx,

    // Time when torrent started.
    start_time: Option<Instant>,

    // Address to listen for incoming connections on.
    listen_address: SocketAddr,

    // Handle for disk task.
    // Option is for moving out of the handle behind a mutable ref.
    disk_handle: Option<tokio::task::JoinHandle<fs::Result<()>>>,

    // Minimum and maximum peers desired for the torrent.
    min_max_peers: (u32, u32),

}

#[derive(Debug)]
pub struct TorrentContext {
    
    // The info hash for this torrent.
    pub info_hash: [u8; 20],

    // The client ID for this client.
    pub client_id: [u8; 20],

    // Allows for peers to pick next block to download.
    pub picker: Picker,

    // Commands to the peer.
    pub torrent_tx: TorrentTx,
    
    // // Commands to disk.
    pub disk_tx: fs::DiskTx,

    // Torrent storage information.
    pub info: StoreInfo,

}

pub struct TorrentConfig {

    // Unique 20-byte identifier used by client.
    pub client_id: [u8; 20],

    // Address on which the client listens for incoming peers.
    pub listen_address: SocketAddr,

    // The minimum and maximum peers desired for the torrent.
    pub min_max_peers: (u32, u32),

    // Path where the torrent will write files.
    pub output_dir: std::path::PathBuf,

}

impl Torrent {

    // This probably shouldnt be async, it is at the moment because Disk::new() is.
    pub async fn new(metainfo: MetaInfo, config: TorrentConfig) -> Self {
        
        let info = StoreInfo::new(&metainfo, config.output_dir);
        let (torrent_tx, torrent_rx) = mpsc::unbounded_channel();
        // Change unwrap after moving disk outside of torrent.
        let (disk_handle, disk_tx) = fs::spawn(info.clone(), metainfo.piece_hashes(), torrent_tx.clone()).await.unwrap();
        
        Torrent {
            ctx: Arc::new(
                TorrentContext {
                    info_hash: metainfo.info_hash(),
                    client_id: config.client_id,
                    picker: Picker::new(
                        info.num_pieces, 
                        info.piece_len,
                        info.last_piece_len
                    ),
                    torrent_tx: torrent_tx.clone(),
                    info,
                    disk_tx,
                }
            ),
            trackers: metainfo.trackers(),
            peers: HashMap::new(),
            available: Vec::new(),
            torrent_rx,
            torrent_tx,
            start_time: None,
            listen_address: config.listen_address,
            disk_handle: Some(disk_handle),
            min_max_peers: config.min_max_peers,
        }
    }

    // Do something with blocks in request queue if there is an error on run.
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("starting torrent");
        self.start_time = Some(Instant::now());
        // Announce start event to trackers.
        self.announce(Some(Event::Started), Instant::now()).await?;
        // Run until there is an error.
        self.run().await?;
        Ok(())
    }

    // TODO: reorder trackers within tiers based on whether we can connect to trackers.
    // TODO: maybe put announces on a seperate task.
    #[tracing::instrument(skip(self, time), fields(num_peers = self.peers.len() + self.available.len()))]
    pub async fn announce(&mut self, event: Option<Event>, time: Instant) -> Result<()> {
        debug_assert!(self.trackers.len() > 0, "no trackers");

        // Use trackers in order of tiers/priority.
        for tier in self.trackers.iter_mut() {
            for tracker in tier {

                //TODO: This is temporary change to handle udp cases properly.
                if tracker.url.as_str().starts_with("udp") {
                    continue;
                }
                
                let num_peers = self.peers.len() + self.available.len();
                // Number of peers we absolutely require.
                let num_peers_essential = if num_peers >= self.min_max_peers.0 as usize || event == Some(Event::Stopped) {
                    None
                } else {
                    Some((self.min_max_peers.1 as usize - num_peers).max(self.min_max_peers.0 as usize))
                };

                // If event OR we need peers and we can announce OR we can have more peers and should announce, then announce.
                if event.is_some() || (num_peers_essential > Some(0) && tracker.can_announce(time)) || tracker.should_announce(time) {
                    
                    let params = AnnounceParams {
                        info_hash: self.ctx.info_hash,
                        peer_id: self.ctx.client_id,
                        // TODO: Change to config.
                        port: 6881,
                        // TODO change all with relation to stats.
                        uploaded: 0,
                        downloaded: 0,
                        left: self.ctx.info.total_len,
                        event,
                        num_want: num_peers_essential,
                        tracker_id: tracker.tracker_id.clone(),
                    };

                    // let peers = tracker.send_announce(params).await?;
                    // self.available.extend(peers.into_iter());
                    // tracker.last_announce = Some(time);
                    match tracker.send_announce(params).await {
                        Ok(peers) => {
                            self.available.extend(peers.into_iter());
                            tracker.last_announce = Some(time);
                        },
                        Err(e) => {
                            tracing::error!("tracker announce error: {}", e);
                        }
                    }

                }

            }
        }

        tracing::trace!("new number of peers: {}", self.peers.len() + self.available.len());
        Ok(())
    }

    fn connect_to_peers(&mut self) {
        let count = self.available.len().min((self.min_max_peers.1 as usize).saturating_sub(self.peers.len()));
        if count == 0 {
            tracing::warn!("no peers to connect to");
            return;
        }

        tracing::info!("connecting to {} peers", count);
        for address in self.available.drain(0..count) {
            let (session, cmd) = PeerSession::new(address, self.ctx.clone());
            self.peers.insert(address, PeerHandle::start_session(session, cmd, None));
        }
    }

    #[tracing::instrument(skip_all, name = "torrent")]
    async fn run(&mut self) -> Result<()> {

        let listener = tokio::net::TcpListener::bind(&self.listen_address).await?;
        self.listen_address = listener.local_addr()?;
        tracing::info!("listening on {}", self.listen_address);

        self.connect_to_peers();
        
        // Top level torrent loop.
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

            Some(cmd) = self.torrent_rx.recv() => {
                match cmd {

                    CommandToTorrent::PeerConnected { address, id } => {
                        if let Some(peer) = self.peers.get_mut(&address) {
                            peer.id = Some(id);
                        }
                    },

                    CommandToTorrent::PieceWritten { idx, valid } => {
                        self.handle_piece_write(idx, valid).await?;
                    },

                    CommandToTorrent::Shutdown => {
                        break;
                    },
                }
            }
        }}

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        
        tracing::info!("disconnecting from {} peers", self.peers.len());
        for peer in self.peers.values() {
            if let Some(tx) = &peer.peer_tx {
                let _ = tx.send(PeerCommand::Shutdown);
            }
        }
        
        for peer in self.peers.values_mut() {
            if let Err(e) = peer
                .session_handle
                .take()
                .expect("missing handle")
                .await
                .expect("peer task err")
            {
                tracing::warn!("session shutdown: {}", e);
            }
        }
        self.ctx.disk_tx.send(fs::CommandToDisk::Shutdown)?;
        self
            .disk_handle
            .take()
            .expect("missing handle")
            .await
            .expect("disk task err")
            .expect("disk task err");

        // Announce stopped event to trackers.
        self.announce(Some(Event::Stopped), Instant::now()).await?;

        self.torrent_tx.send(CommandToTorrent::Shutdown)?;
        Ok(())
    }

    async fn handle_piece_write(&mut self, idx: usize, valid: bool) -> Result<()> {
        
        if valid {
            
            self.ctx.picker.partial_pieces.write().await.remove(&idx);
            self.ctx.picker.piece_picker.write().await.received_piece(idx);
            
            let num_pieces_missing = self.ctx.picker.piece_picker.read().await.own_bitfield().count_zeros();
            tracing::info!("piece {} downloaded, {} pieces remain", idx, num_pieces_missing);

            for peer in self.peers.values() {
                if let Some(tx) = &peer.peer_tx {
                    tx.send(PeerCommand::PieceWritten(idx)).ok();
                }
            }

            // Check if torrent is fully downloaded.
            if num_pieces_missing == 0 {
                tracing::info!("torrent download complete");
                // Shutdown everything.
                self.shutdown().await?;
            }
        
        } else {
            // Free all blocks in piece.
            // TODO: Punish peer in some way.
            if let Some(piece) = self.ctx.picker.partial_pieces.read().await.get(&idx) {
                piece.write().await.free_all_blocks();
            }
        }

        Ok(())
    }
}
