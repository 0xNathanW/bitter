use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

pub mod session;
pub mod peer;
mod state;
mod message;
mod handshake;
mod request;

pub type Result<T, E = PeerError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum PeerError {

    #[error("peer IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("peer Timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("peer handshake provided incorrect protocol")]
    IncorrectProtocol,

    #[error("peer handshake provided incorrect info hash")]
    IncorrectInfoHash,

    #[error("invalid message ID: {0}")]
    InvalidMessageId(u8),

    #[error("peer channel error")]
    Channel,

    #[error("bitfield sent before handshake")]
    UnexpectedBitfield,
}

impl<T> From<SendError<T>> for PeerError {
    fn from(_: SendError<T>) -> Self {
        PeerError::Channel        
    }
}
