#![allow(dead_code, unused_imports)]

use thiserror::Error;

pub mod torrent;
pub mod piece;
pub mod tracker;
pub mod p2p;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    
    #[error("bencoding error: {0}")]
    BencodeError(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
 
    #[error("piece idx {0} out of bounds")]
    InvalidPieceIdx(usize),
}
