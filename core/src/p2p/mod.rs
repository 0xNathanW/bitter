use thiserror::Error;
use tokio::{
    sync::mpsc::error::SendError,
    time::error::Elapsed,
};

pub mod peer;
mod message;
mod handshake;
mod manage;
mod request;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Handshake error: {0}")]
    Handshake(#[from] handshake::Error),
    
    #[error("Invalid message id recieved: {0}")]
    InvalidMessageID(u8),

    #[error("Unexpected message recieved: expected {0}, got {1}")]
    UnexpectedMessage(String, String),
    
    #[error("Attempted to read/write to a closed stream")]
    NoStream,
    
    #[error("Peer choked, unable to send requests")]
    Choke,
    
    #[error(transparent)]
    PieceError(#[from] piece::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("Channel Error: {0}")]
    ChannelError(#[from] tokio::sync::mpsc::error::SendError<piece::PieceData>),
}

use crate::tracker::PeerInfo;
pub fn parse_peers(raw: Vec<PeerInfo>) -> Vec<peer::Peer> {
    raw.into_iter().map(|p| peer::Peer::new(p.id, p.addr)).collect()
}
