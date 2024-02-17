use tokio::sync::mpsc::{self, UnboundedSender};

mod session;
mod state;
mod message;
mod handshake;

pub use session::PeerSession;

type Result<T, E = PeerError> = std::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum PeerError {

    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("handshake provided incorrect protocol")]
    IncorrectProtocol,

    #[error("handshake provided incorrect info-hash")]
    IncorrectInfoHash,

    #[error("no handshake recieved")]
    NoHandshake,    

    #[error("invalid message ID: {0}")]
    InvalidMessageId(u8),

    #[error("peer channel error")]
    Channel,

    #[error("bitfield sent before handshake")]
    UnexpectedBitfield,

    #[error("no message recieved")]
    NoMessage,

    #[error("connection timeout")]
    Timeout,
}

impl<T> From<mpsc::error::SendError<T>> for PeerError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        PeerError::Channel        
    }
}

// Commands that can be sent to a peer.
pub enum PeerCommand {

    // Tell the peer we got a piece (piece idx).
    PieceWritten(usize),

    // End the peer session safely.
    Shutdown,

}

#[derive(Debug)]
pub struct PeerHandle {

    // Unique 20-byte id for peer.
    pub id: Option<[u8; 20]>,

    // Sends commands to the torrent.
    pub peer_tx: Option<UnboundedSender<PeerCommand>>,

    // Handle to the peer session.
    pub session_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    
}

impl PeerHandle {

    fn new(peer_tx: UnboundedSender<PeerCommand>, handle: tokio::task::JoinHandle<Result<()>>) -> PeerHandle {
        PeerHandle {
            id: None,
            peer_tx: Some(peer_tx),
            session_handle: Some(handle),
        }
    }

    pub fn start_session(
        mut session: PeerSession,
        peer_tx: UnboundedSender<PeerCommand>,
        socket: Option<tokio::net::TcpStream>
    ) -> PeerHandle {
        let handle = tokio::spawn(async move {
            session.start_session(socket).await
        });
        PeerHandle::new(peer_tx, handle)
    }
}