use thiserror::Error;

mod peer;
mod bitfield;
mod message;
mod handshake;

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
}

use crate::tracker::PeerInfo;
pub fn parse_peers(raw: Vec<PeerInfo>) -> Vec<peer::Peer> {
    todo!()
}