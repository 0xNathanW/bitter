use std::{
    collections::HashMap, 
    net::{Ipv4Addr, SocketAddr}, 
    sync::Arc, time::{Duration, Instant}
};
use tokio::{net::TcpListener, sync::mpsc, time};
use crate::{
    config::Config, 
    disk::{AllocationError, DiskTx}, 
    p2p::{state::{ConnState, SessionState}, PeerCommand, PeerHandle, PeerSession}, 
    picker::Picker, 
    stats::{PeerStats, PieceStats, ThroughputStats, TorrentStats}, 
    store::TorrentInfo, 
    tracker::{AnnounceParams, Event, Tracker, TrackerError}, 
    Bitfield, 
    TorrentID, 
    UserCommand, 
    UserTx
};

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {

    #[error(transparent)]
    AllocationError(#[from] AllocationError),

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

pub enum TorrentCommand {
    
    // Sent by client after disk allocation.
    Bitfield(Bitfield),
    
    // Sent by disk task when piece written.
    PieceWritten { idx: usize, valid: bool },
    
    PeerState { address: SocketAddr, state: SessionState },

    // Sent by itself to shutdown.
    Shutdown,
    
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum TorrentState {
    #[default]
    Checking,
    Announcing,
    Downloading,
    Seeding,
    Stopped,
    Paused,
}

// Type aliases.
pub type Result<T> = std::result::Result<T, TorrentError>;
pub type TorrentTx = mpsc::UnboundedSender<TorrentCommand>;
pub type TorrentRx = mpsc::UnboundedReceiver<TorrentCommand>;

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

    user_tx: UserTx,

    start_time: Option<Instant>,

    run_duration: Duration,

    throughput: ThroughputStats,

    state: TorrentState,

    listen_port: u16,

    config: Config,

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
    pub disk_tx: DiskTx,

    // Torrent storage information.
    pub info: TorrentInfo,

}

pub struct TorrentParams {

    pub id: TorrentID,

    pub info: TorrentInfo,

    pub info_hash: [u8; 20],

    pub client_id: [u8; 20],

    pub trackers: Vec<Vec<Tracker>>,

    pub user_tx: UserTx,

    pub disk_tx: DiskTx,

    pub listen_port: u16,

    pub config: Config,

}

impl Torrent {

    pub fn new(params: TorrentParams) -> (Self, TorrentTx) {        
        
        let num_pieces = params.info.num_pieces;
        let piece_len = params.info.piece_len;
        let last_piece_len = params.info.last_piece_len;
        let (torrent_tx, torrent_rx) = mpsc::unbounded_channel();
        
        (
            Torrent {
                ctx: Arc::new(
                    TorrentContext {
                        id: params.id,
                        info_hash: params.info_hash,
                        client_id: params.client_id,
                        picker: Picker::new(
                            num_pieces, 
                            piece_len,
                            last_piece_len,
                        ),
                        torrent_tx: torrent_tx.clone(),
                        info: params.info,
                        disk_tx: params.disk_tx,
                    }
                ),
                trackers: params.trackers,
                peers: HashMap::new(),
                available: Vec::new(),
                user_tx: params.user_tx,
                torrent_rx,
                start_time: None,
                run_duration: Duration::default(),
                throughput: ThroughputStats::default(),
                state: TorrentState::Checking,
                listen_port: params.listen_port,
                config: params.config,
            },
            torrent_tx
        )
    }

    // TODO: do something with blocks in request queue if there is an error on run.
    #[tracing::instrument(skip_all, name = "torrent", fields(id = %hex::encode(self.ctx.info_hash)))]
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("starting torrent");

        // Wait for disk allocation result, set own bitfield to pieces we already have.
        match self.torrent_rx.recv().await.ok_or(TorrentError::ChannelError("torrent tx dropped".to_string()))? {
            TorrentCommand::Bitfield(bitfield) => {
                tracing::info!("own bitfield has {}/{} pieces", bitfield.count_ones(), self.ctx.info.num_pieces);
                self.ctx.picker.piece_picker.write().await.set_own_bitfield(bitfield);
            },
            TorrentCommand::Shutdown => return Ok(()),
            _ => unreachable!("unexpected command for allocation"),
        }

        self.start_time = Some(Instant::now());
        // TODO: Send user msg if there is error.
        self.run().await?;

        Ok(())
    }

    // TODO: reorder trackers within tiers based on whether we can connect to trackers.
    // TODO: maybe put announces on a seperate task.
    #[tracing::instrument(skip(self, time), fields(num_peers = self.peers.len() + self.available.len()))]
    async fn announce(&mut self, event: Option<Event>, time: Instant) -> Result<()> {
        self.state = TorrentState::Announcing;

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
                    
                    let left = self.ctx.info.total_len - 
                        (
                            self.ctx.picker.piece_picker
                                .read()
                                .await
                                .own_bitfield()
                                .iter_ones()
                                .fold(0, |acc, idx| acc + self.ctx.info.piece_len(idx)) 
                                as u64
                        );
                    tracing::debug!("announce bytes left calculated: {}", left);
                    
                    let params = AnnounceParams {
                        info_hash: self.ctx.info_hash,
                        peer_id: self.ctx.client_id,
                        // CHANGE
                        port: self.listen_port,
                        uploaded: self.throughput.up.total(),
                        downloaded: self.throughput.down.total(),
                        left,
                        event,
                        num_want: num_peers_essential,
                        tracker_id: tracker.tracker_id.clone(),
                    };

                    match tracker.send_announce(params).await {
                        Ok(peers) => {
                            tracing::info!("{} provided {} peers", tracker.url, peers.len());
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

    async fn run(&mut self) -> Result<()> {
        let mut ticker = time::interval(time::Duration::from_secs(1));
        let mut last_tick = None;
        
        // Initial announce.
        self.announce(Some(Event::Started), Instant::now()).await?;
        
        // Start listening for incoming connections.
        let listen_address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), self.listen_port);
        tracing::info!("attempting to listen on {:?}", listen_address);
        let listener = TcpListener::bind(listen_address).await?;
        tracing::info!("listening on {:?}", listen_address);
        
        self.state = if self.ctx.picker.piece_picker.read().await.own_bitfield().count_ones() == self.ctx.info.num_pieces as usize {
            TorrentState::Seeding
        } else {
            TorrentState::Downloading
        };
        
        if self.state == TorrentState::Downloading {
            self.connect_to_peers();
        }

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

                    // From Client.
                    TorrentCommand::Bitfield(bitfield) => {
                        self.ctx.picker.piece_picker.write().await.set_own_bitfield(bitfield);
                    },

                    // From peers.
                    TorrentCommand::PeerState { address, state } => {
                        self.handle_peer_state(address, state);
                    },

                    // From disk.
                    TorrentCommand::PieceWritten { idx, valid } => {
                        self.handle_piece_write(idx, valid).await?;
                    },

                    TorrentCommand::Shutdown => {
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
            peer.peer_tx.send(PeerCommand::Shutdown).ok();
        }
        
        for peer in self.peers.values_mut() {
            if let Err(e) = peer
                .session_handle
                .take()
                .expect("missing handle")
                .await
            {
                tracing::warn!("session shutdown: {}", e);
            }
        }
        
        // Announce completed event to trackers.
        self.announce(Some(Event::Completed), Instant::now()).await?;
        self.user_tx.send(crate::UserCommand::TorrentResult {
            id: self.ctx.id,
            result: Ok(()),
        })?;
        Ok(())
    }

    async fn handle_piece_write(&mut self, idx: usize, valid: bool) -> Result<()> {
        
        if valid {
            
            self.ctx.picker.partial_pieces.write().await.remove(&idx);
            self.ctx.picker.piece_picker.write().await.received_piece(idx);
            
            let num_pieces_missing = self.ctx.picker.piece_picker.read().await.own_bitfield().count_zeros();
            tracing::info!("piece {} downloaded, {} pieces remain", idx, num_pieces_missing);

            for peer in self.peers.values() {
                peer.peer_tx.send(PeerCommand::PieceWritten(idx)).ok();
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
        // TODO: is this even necessary?
        let elapsed_since_tick = last_tick
            .or(self.start_time)
            .map(|t| time.saturating_duration_since(t))
            .unwrap_or_default();
        self.run_duration += elapsed_since_tick;
        *last_tick = Some(time);

        let stats = self.build_stats().await;
        self.user_tx.send(UserCommand::TorrentStats {
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
            state: self.state,
            throughput: self.throughput,
            peer_stats,
        }
    }
}
