use std::{
    collections::HashMap, net::SocketAddr, sync::Arc, time::{Duration, Instant}
};
use tokio::{sync::mpsc, time};
use crate::{
    config::TorrentConfig, 
    fs, 
    metainfo::MetaInfo,
    p2p::{state::{ConnState, SessionState}, PeerCommand, PeerHandle, PeerSession},
    picker::Picker,
    stats::{PeerStats, PieceStats, ThroughputStats, TorrentStats},
    store::StoreInfo,
    tracker::{AnnounceParams, Event, Tracker, TrackerError},
    CommandToUser,
    TorrentID,
    UserTx,
};

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {

    #[error("tracker error: {0}")]
    TrackerError(#[from] TrackerError),
    
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("channel error: {0}")]
    ChannelError(String),
}

impl<T> From<mpsc::error::SendError<T>> for TorrentError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        TorrentError::ChannelError(e.to_string())
    }
}

pub enum CommandToTorrent {
    
    // Sent by peer task when peer successfully connects.
    PeerConnected { address: SocketAddr, id: [u8; 20] },
    
    // Sent by disk task when piece written.
    PieceWritten { idx: usize, valid: bool },
    
    PeerState { address: SocketAddr, state: SessionState },

    // Sent by itself to shutdown.
    Shutdown,
    
}

// Type aliases.
pub type Result<T> = std::result::Result<T, TorrentError>;
pub type TorrentTx = mpsc::UnboundedSender<CommandToTorrent>;
pub type TorrentRx = mpsc::UnboundedReceiver<CommandToTorrent>;

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

    torrent_rx: TorrentRx,

    torrent_tx: TorrentTx,

    user_tx: UserTx,

    start_time: Option<Instant>,

    run_duration: Duration,

    throughput: ThroughputStats,

    config: TorrentConfig,

}

#[derive(Debug)]
pub struct TorrentContext {
    
    // The unique identifier for this torrent.
    pub id: TorrentID,

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

pub struct TorrentParams {

    pub id: TorrentID,

    pub client_id: [u8; 20],

    pub metainfo: MetaInfo,

    pub user_tx: UserTx,

    pub disk_tx: fs::DiskTx,

    pub config: TorrentConfig,

}

impl Torrent {

    // This probably shouldnt be async, it is at the moment because Disk::new() is.
    pub fn new(params: TorrentParams) -> (Self, TorrentTx) {        
        
        let info = StoreInfo::new(&params.metainfo, params.config.output_dir.clone());
        let num_pieces = info.num_pieces;
        let piece_len = info.piece_len;
        let last_piece_len = info.last_piece_len;
        let (torrent_tx, torrent_rx) = mpsc::unbounded_channel();
        
        (
            Torrent {
                ctx: Arc::new(
                    TorrentContext {
                        id: params.id,
                        info_hash: params.metainfo.info_hash(),
                        client_id: params.client_id,
                        picker: Picker::new(
                            num_pieces, 
                            piece_len,
                            last_piece_len,
                        ),
                        torrent_tx: torrent_tx.clone(),
                        info,
                        disk_tx: params.disk_tx,
                    }
                ),
                trackers: params.metainfo.trackers(),
                peers: HashMap::new(),
                available: Vec::new(),
                user_tx: params.user_tx,
                torrent_rx,
                torrent_tx: torrent_tx.clone(),
                start_time: None,
                run_duration: Duration::default(),
                throughput: ThroughputStats::default(),
                config: params.config,
            },
            torrent_tx
        )
    }

    // Do something with blocks in request queue if there is an error on run.
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("starting torrent");
        self.start_time = Some(Instant::now());
        // Send user msg if there is error.
        self.run().await.map_err(|e| {
            self.user_tx.send(CommandToUser::TorrentError(e.to_string())).ok();
            e
        })?;
        Ok(())
    }

    // TODO: reorder trackers within tiers based on whether we can connect to trackers.
    // TODO: maybe put announces on a seperate task.
    #[tracing::instrument(skip(self, time), fields(num_peers = self.peers.len() + self.available.len()))]
    async fn announce(&mut self, event: Option<Event>, time: Instant) -> Result<()> {

        // Use trackers in order of tiers/priority.
        for tier in self.trackers.iter_mut() {
            for tracker in tier {

                //TODO: This is temporary change to handle udp cases properly.
                if tracker.url.as_str().starts_with("udp") {
                    continue;
                }
                
                let num_peers = self.peers.len() + self.available.len();
                // Number of peers we absolutely require.
                let num_peers_essential = if num_peers >= self.config.min_max_peers.0 as usize || event == Some(Event::Stopped) {
                    None
                } else {
                    Some((self.config.min_max_peers.1 as usize - num_peers).max(self.config.min_max_peers.0 as usize))
                };

                // If event OR we need peers and we can announce OR we can have more peers and should announce, then announce.
                if event.is_some() || (num_peers_essential > Some(0) && tracker.can_announce(time)) || tracker.should_announce(time) {
                    
                    let params = AnnounceParams {
                        info_hash: self.ctx.info_hash,
                        peer_id: self.ctx.client_id,
                        port: self.config.listen_address.port() as u16,
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
        let count = self.available.len().min((self.config.min_max_peers.1 as usize).saturating_sub(self.peers.len()));
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
        let mut ticker = time::interval(time::Duration::from_secs(1));
        let mut last_tick = None;
        
        // Initial announce.
        self.announce(Some(Event::Started), Instant::now()).await?;

        let listener = tokio::net::TcpListener::bind(&self.config.listen_address).await?;
        debug_assert_eq!(listener.local_addr()?, self.config.listen_address);
        tracing::info!("listening on {}", self.config.listen_address);

        self.connect_to_peers();
        
        // Top level torrent loop.
        loop { tokio::select! {

            now = ticker.tick() => self.tick(&mut last_tick, now.into_std()).await?,

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

                    CommandToTorrent::PeerState { address, state } => {
                        self.handle_peer_state(address, state);
                    }

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
        
        // Announce stopped event to trackers.
        self.announce(Some(Event::Stopped), Instant::now()).await?;
        self.user_tx.send(crate::CommandToUser::TorrentComplete(self.ctx.id))?;
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

    fn handle_peer_state(&mut self, address: SocketAddr, state: SessionState) {
        if let Some(peer) = self.peers.get_mut(&address) {
            peer.state = state;
            self.throughput += &state.throughput;

            if peer.state.conn_state == ConnState::Disconnected {
                self.peers.remove(&address);
            }
        } else {
            tracing::warn!("peer not found: {}", address);
        }
    }

    async fn tick(&mut self, last_tick: &mut Option<Instant>, time: Instant) -> Result<()> {

        let elapsed_since_tick = last_tick
            .or(self.start_time)
            .map(|t| time.saturating_duration_since(t))
            .unwrap_or_default();
        self.run_duration += elapsed_since_tick;
        *last_tick = Some(time);

        let stats = self.build_stats().await;
        self.user_tx.send(CommandToUser::TorrentStats {
            id: self.ctx.id,
            stats,
        })?;
        self.throughput.reset();

        Ok(())
    }

    async fn build_stats(&mut self) -> TorrentStats {

        let num_pieces = self.ctx.info.num_pieces as usize;
        let num_downloaded = self.ctx.picker.piece_picker.read().await.own_bitfield().count_ones();
        let num_pending = self.ctx.picker.partial_pieces.read().await.len();

        let peer_stats = self.peers
            .iter()
            .map(|(address, peer)| PeerStats {
                address: *address,
                state: peer.state,
                num_pieces: peer.state.num_pieces,
                throughput: peer.state.throughput,
            })
            .collect();

        TorrentStats {
            start_time: self.start_time,
            time_elapsed: self.run_duration,
            piece_stats: PieceStats {
                num_pieces,
                num_pending,
                num_downloaded,                
            },
            throughput: self.throughput,
            peer_stats,
        }
    }
}
