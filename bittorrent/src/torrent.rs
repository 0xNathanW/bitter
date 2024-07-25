use std::{
    collections::HashMap, 
    net::{Ipv4Addr, SocketAddr}, 
    sync::Arc, time::Instant,
};
use tokio::{net::TcpListener, sync::{mpsc, oneshot}, time};
use tracing::Instrument;
use url::Url;
use crate::{
    config::Config, 
    disk::{AllocationError, DiskTx}, 
    info::TorrentInfo, 
    p2p::{state::{ConnState, SessionState}, PeerCommand, PeerHandle},
    picker::Picker,
    stats::{PeerStats, PieceStats, ThroughputStats, TorrentStats},
    tracker::{AnnounceParams, Event, TrackersHandle},
    Bitfield,
    UserCommand,
    UserTx,
    ID,
};

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {

    #[error(transparent)]
    AllocationError(#[from] AllocationError),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    // When sends to disks fail.
    #[error("disk failure")]
    DiskFailure

}

#[derive(Debug)]
pub enum TorrentCommand {
    
    // Sent by disk task when piece written.
    PieceWritten { idx: usize, valid: bool },
    
    // Sent by peers to update state.
    PeerState { address: SocketAddr, state: SessionState },

    // Sent by trackers to update peer list.
    Peers(Vec<SocketAddr>),

    // Sent by itself or client to shutdown.
    Shutdown,
    
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum TorrentState {
    #[default]
    Checking,
    Downloading,
    Seeding,
    Stopped,
    Paused,
}

// Type aliases.
pub type Result<T> = std::result::Result<T, TorrentError>;
pub type TorrentTx = mpsc::UnboundedSender<TorrentCommand>;
pub type TorrentRx = mpsc::UnboundedReceiver<TorrentCommand>;

pub struct TorrentHandle {

    pub torrent_tx: TorrentTx,

    pub handle: tokio::task::JoinHandle<()>,

}

impl TorrentHandle {

    pub fn start_torrent(
        params: TorrentParams,
        rx: oneshot::Receiver<std::result::Result<Bitfield, AllocationError>>,
    ) -> Self {
        
        let info_hash = params.info_hash;
        let (mut torrent, torrent_tx) = Torrent::new(params);

        let handle = tokio::task::spawn(async move { 
            if let Err(e) = torrent.start(rx).await {
                tracing::error!("torrent error: {}", e);
            }
            torrent.shutdown().await;
        }.instrument(tracing::info_span!("torrent", id = %hex::encode(info_hash))));
        
        TorrentHandle {
            torrent_tx,
            handle,
        }
    }

}

#[derive(Debug)]
pub struct TorrentContext {

    pub info_hash: ID,

    pub client_id: ID,

    pub picker: Picker,

    pub torrent_tx: TorrentTx,
    
    pub disk_tx: DiskTx,

    pub info: TorrentInfo,

}

pub struct TorrentParams {

    pub info: TorrentInfo,

    pub info_hash: ID,

    pub client_id: ID,

    pub tracker_urls: Vec<Vec<Url>>,

    pub user_tx: UserTx,

    pub disk_tx: DiskTx,

    pub listen_port: u16,

    pub config: Config,

}

struct Torrent {

    torrent_rx: TorrentRx,

    // Context is a read-only state accessible by peers in threads.
    ctx: Arc<TorrentContext>,

    // Peers we have active sessions with.
    peers: HashMap<SocketAddr, PeerHandle>,
    
    // Peers we know about but don't have a session with.
    available: Vec<SocketAddr>,

    trackers: TrackersHandle,

    user_tx: UserTx,

    throughput: ThroughputStats,

    state: TorrentState,

    listen_port: u16,

    config: Config,

}

impl Torrent {

    pub fn new(params: TorrentParams) -> (Self, TorrentTx) {        
        
        let (torrent_tx, torrent_rx) = mpsc::unbounded_channel();

        (
            Torrent {
                ctx: Arc::new(
                    TorrentContext {
                        info_hash: params.info_hash,
                        client_id: params.client_id,
                        picker: Picker::new(
                            params.info.num_pieces,
                            params.info.piece_len,
                            params.info.last_piece_len,
                        ),
                        torrent_tx: torrent_tx.clone(),
                        info: params.info,
                        disk_tx: params.disk_tx,
                    }
                ),
                trackers: TrackersHandle::new(params.tracker_urls),
                peers: HashMap::new(),
                available: Vec::new(),
                user_tx: params.user_tx,
                torrent_rx,
                throughput: ThroughputStats::default(),
                state: TorrentState::Checking,
                listen_port: params.listen_port,
                config: params.config,
            },
            torrent_tx
        )
    }

    // TODO: do something with blocks in request queue if there is an error on run.
    pub async fn start(
        &mut self, 
        rx: oneshot::Receiver<std::result::Result<Bitfield, AllocationError>>
    ) -> Result<()> {

        // Wait for disk allocation result, set own bitfield to pieces we already have.
        let bf = rx.await.map_err(|_| TorrentError::DiskFailure)??;
        tracing::info!("own bitfield has {}/{} pieces", bf.count_ones(), self.ctx.info.num_pieces);
        if bf.any() {
            self.ctx.picker.pieces.write().await.set_own_bitfield(bf);
        }

        self.run().await?;
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        
        let start_time = Instant::now();
        let mut ticker = time::interval(time::Duration::from_secs(1));
        
        // Start listening for incoming connections.
        let listen_address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), self.listen_port);
        let listener = TcpListener::bind(listen_address).await?;
        tracing::info!("listening on {:?}", listen_address);
        
        self.trackers.start(self.ctx.torrent_tx.clone()).await;
        self.announce(Some(Event::Started)).await;

        // Are we seeding or downloading?
        // TODO: check.
        self.state = if self.ctx.picker.pieces.read().await.all() {
            TorrentState::Seeding
        } else {
            TorrentState::Downloading
        };

        loop { tokio::select! {

            now = ticker.tick() => self.tick(start_time, now.into_std()).await,

            // Accept incoming peer connections.
            new_peer_conn = listener.accept() => {
                match new_peer_conn {
                    Ok((stream, address)) => {
                        self.peers.insert(address, PeerHandle::start_session(address, self.ctx.clone(), Some(stream)));
                    },
                    Err(e) => tracing::warn!("inbound peer connection error: {}", e),
                };
            },

            Some(cmd) = self.torrent_rx.recv() => {
                match cmd {

                    // From peers.
                    TorrentCommand::PeerState { address, state } => self.handle_peer_state(address, state).await,

                    // From disk.
                    TorrentCommand::PieceWritten { idx, valid } => self.handle_piece_write(idx, valid).await,

                    // From trackers.
                    TorrentCommand::Peers(peers) => {
                        self.available.extend(peers);
                        self.manage_peer_nums().await;
                    },

                    TorrentCommand::Shutdown => break,
                }
            }
        }}

        Ok(())
    }

    async fn shutdown(&mut self) {
        
        for peer in self.peers.values() {
            peer.peer_tx.send(PeerCommand::Shutdown).ok();
        }
        
        for (addr, peer) in self.peers.drain() {
            if let Err(e) = peer.session_handle.await {
                tracing::error!("peer task {} panicked: {}", addr, e);
            }
        }
        
        // Announce completed event to trackers.
        self.trackers.shutdown().await;
        self.user_tx.send(crate::UserCommand::TorrentResult {
            id: self.ctx.info_hash,
            result: Ok(()),
        }).ok();
    }

    async fn manage_peer_nums(&mut self) {

        let count_to_max = self.config.max_peers - self.peers.len();
        let connect_count = count_to_max.min(self.available.len());
        tracing::info!("attempting to connect to {} peers", connect_count); 
        // If there is enough in available, connect to max, otherwise connect to as many possible and announce the number remaining.
        for address in self.available.drain(..connect_count) {
            self.peers.insert(address, PeerHandle::start_session(address, self.ctx.clone(), None));
        }
        if self.peers.len() == self.config.max_peers as usize {
            tracing::info!("max peers reached");
            self.trackers.tracker_tx.send(None).ok();
        } else {
            self.announce(None).await;
        }

    }

    async fn announce(&mut self, event: Option<Event>) {

        tracing::info!("announcing to trackers");
        let left = self.ctx.info.total_len - 
        (
            self.ctx.picker.pieces
                .read()
                .await
                .own_bitfield()
                .iter_ones()
                .fold(0, |acc, idx| acc + self.ctx.info.piece_len(idx)) 
                as u64
        );
        
        let params = AnnounceParams {
            info_hash: self.ctx.info_hash,
            client_id: self.ctx.client_id,
            port: self.listen_port,
            uploaded: self.throughput.up.total(),
            downloaded: self.throughput.down.total(),
            left,
            event,
            num_want: None, // Default 50.
        };
        tracing::debug!("announce: {:#?}", params);

        // If we have no peers and no trackers, shutdown.
        if let Err(_) = self.trackers.tracker_tx.send(Some(params)) {
            if self.peers.is_empty() {
                tracing::warn!("no peers and no trackers, shutting down");
                let _ = self.ctx.torrent_tx.send(TorrentCommand::Shutdown);
            }
        };
    }

    async fn handle_piece_write(&mut self, idx: usize, valid: bool) {
        if valid {
            self.ctx.picker.partial_pieces.write().await.remove(&idx);
            self.ctx.picker.pieces.write().await.received_piece(idx);
            
            let num_pieces_missing = self.ctx.picker.pieces.read().await.own_bitfield().count_zeros();
            tracing::info!("piece {} downloaded, {} pieces remain", idx, num_pieces_missing);

            for peer in self.peers.values() {
                let _ = peer.peer_tx.send(PeerCommand::PieceWritten(idx));
            }

            // Check if torrent is fully downloaded.
            if num_pieces_missing == 0 {
                tracing::info!("torrent download complete");
                let _ = self.ctx.torrent_tx.send(TorrentCommand::Shutdown);
            }
        
        } else {
            // Free all blocks in piece.
            // TODO: Punish peer in some way.
            if let Some(piece) = self.ctx.picker.partial_pieces.read().await.get(&idx) {
                piece.write().await.free_all_blocks();
            }
        }
    }

    // Also handles disconnections.
    async fn handle_peer_state(&mut self, address: SocketAddr, state: SessionState) {
        if let Some(peer) = self.peers.get_mut(&address) {
            peer.state = state;
            self.throughput += &state.throughput;
            if peer.state.conn_state == ConnState::Disconnected {
                self.peers.remove(&address);
                self.manage_peer_nums().await;
            }

        } else {
            tracing::warn!("peer not found: {}", address);
        }
    }

    async fn tick(&mut self, start_time: Instant, now: Instant) {

        let time_elapsed = now.duration_since(start_time);
        let num_pieces = self.ctx.info.num_pieces as usize;
        let num_downloaded = self.ctx.picker.pieces.read().await.own_bitfield().count_ones();
        let num_pending = self.ctx.picker.partial_pieces.read().await.len();

        // Collate stats from peers.
        let peer_stats = self.peers
            .iter()
            .map(|(address, peer)| PeerStats {
                address: *address,
                state: peer.state,
            })
            .collect();

        let stats = TorrentStats {
            start_time,
            time_elapsed,
            piece_stats: PieceStats {
                num_pieces,
                num_pending,
                num_downloaded,
            },
            state: self.state,
            throughput: self.throughput,
            peer_stats,
        };

        let _ = self.user_tx.send(UserCommand::TorrentStats {
            id: self.ctx.info_hash,
            stats,
        });
        self.throughput.reset();
    }
}
