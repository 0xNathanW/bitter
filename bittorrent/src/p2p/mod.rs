use std::{net::SocketAddr, sync::Arc};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::Instrument;
use crate::{block::Block, torrent::TorrentContext};

mod session;
mod message;
mod handshake;
pub mod state;

pub use session::PeerSession;
use state::SessionState;

type Result<T> = std::result::Result<T, PeerError>;
type PeerRx = mpsc::UnboundedReceiver<PeerCommand>;
pub type PeerTx = mpsc::UnboundedSender<PeerCommand>;

#[derive(thiserror::Error, Debug)]
pub enum PeerError {

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("handshake provided incorrect protocol")]
    IncorrectProtocol,

    #[error("handshake provided incorrect info-hash")]
    IncorrectInfoHash,

    #[error("no handshake recieved")]
    NoHandshake,

    #[error("invalid message ID: {0}")]
    InvalidMessageId(u8),

    #[error("bitfield sent before handshake")]
    UnexpectedBitfield,

    #[error("invalid message payload")]
    InvalidMessage,

    #[error("connection timeout")]
    Timeout,
}

// Commands that can be sent to a peer.
pub enum PeerCommand {

    // Tell the peer we got a piece (piece idx).
    PieceWritten(usize),

    // Block read from disk.
    BlockRead(Block),

    Shutdown,

}

#[derive(Debug)]
pub struct PeerHandle {

    // Sends commands to the torrent.
    pub peer_tx: PeerTx,

    // Handle to the peer session.
    pub session_handle: JoinHandle<()>,

    // Tracks the state of the peer session.
    pub state: SessionState,
    
}

impl PeerHandle {
    pub fn start_session(
        address: SocketAddr,
        ctx: Arc<TorrentContext>,
        socket: Option<tokio::net::TcpStream>
    ) -> Self {

        let (mut session, peer_tx) = PeerSession::new(address, ctx);
        let session_handle = tokio::spawn(async move {
            if let Err(e) = session.start_session(socket).await {
                tracing::error!("session error: {}", e);
            }
            session.disconnect().await;
        }.instrument(tracing::info_span!("peer", addr = %address)));

        PeerHandle {
            peer_tx,
            session_handle,
            state: SessionState::default(),
        }
    }
}