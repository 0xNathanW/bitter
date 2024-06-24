use tokio::{sync::mpsc, task::JoinHandle};
use tracing::span;
use crate::block::Block;

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

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("handshake provided incorrect protocol")]
    IncorrectProtocol,

    #[error("handshake provided incorrect info-hash")]
    IncorrectInfoHash,

    #[error("no handshake recieved")]
    NoHandshake,    

    #[error("invalid message ID: {0}")]
    InvalidMessageId(u8),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("bitfield sent before handshake")]
    UnexpectedBitfield,

    #[error("invalid message payload")]
    InvalidMessage,

    #[error("connection timeout")]
    Timeout,
}

impl<T> From<mpsc::error::SendError<T>> for PeerError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        PeerError::Channel(e.to_string())
    }
}

// Commands that can be sent to a peer.
pub enum PeerCommand {

    // Tell the peer we got a piece (piece idx).
    PieceWritten(usize),

    // Block read from disk.
    BlockRead(Block),

    // End the peer session safely.
    Shutdown,

}

#[derive(Debug)]
pub struct PeerHandle {

    // Sends commands to the torrent.
    pub peer_tx: PeerTx,

    // Handle to the peer session.
    pub session_handle: Option<JoinHandle<Result<()>>>,

    // Tracks the state of the peer session.
    pub state: SessionState,
    
}

impl PeerHandle {
    pub fn start_session(
        mut session: PeerSession,
        peer_tx: PeerTx,
        socket: Option<tokio::net::TcpStream>
    ) -> Self {
        let session_handle = Some(tokio::spawn(async move {
            let _guard = span!(tracing::Level::INFO, "peer", "addr" = %session.address);
            let session_result = session.start_session(socket)
                .await
                .map_err(|e| {tracing::error!("session error: {}", e); e});
            session.disconnect().await;
            session_result
        }));
        PeerHandle {
            peer_tx,
            session_handle,
            state: SessionState::default(),
        }
    }
}