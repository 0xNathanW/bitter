use thiserror::Error;
use crate::piece;

pub mod peer;
mod bitfield;
mod message;
mod handshake;
mod manage;
mod request;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Attempted to read/write to a closed stream")]
    NoStream,

    #[error("Handshake error: {0}")]
    Handshake(#[from] handshake::Error),

    #[error("Invalid message id recieved: {0}")]
    InvalidMessageID(u8),

    #[error("Unexpected message recieved: expected {0}, got {1}")]
    UnexpectedMessage(String, String),

    #[error(transparent)]
    PieceError(#[from] piece::Error),

    #[error("Peer choked: unable to send requests")]
    Choke,

    #[error("Channel Error: {0}")]
    ChannelError(#[from] tokio::sync::mpsc::error::SendError<piece::PieceData>),
}

use crate::tracker::PeerInfo;
pub fn parse_peers(raw: Vec<PeerInfo>) -> Vec<peer::Peer> {
    raw.into_iter().map(|p| peer::Peer::new(p.id, p.addr)).collect()
}
