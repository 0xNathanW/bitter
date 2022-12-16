use thiserror::Error;
use sha1::{Sha1, Digest};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid hash for piece {idx}: expected {expected:?}, got {actual:?}")]
    InvalidHash { idx: u32, expected: [u8; 20], actual: [u8; 20] },

    #[error("Invalid index for piece: expected {0}, got {1}")]
    InvalidIndex(u32, u32),
}

// Piece of the torrent data.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub idx:    u32,
    pub hash:   [u8; 20],
    pub begin:  u32,
    pub end:    u32,
}

impl Piece {
    pub fn verify_hash(&self, data: &[u8]) -> Result<(), Error> {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let hash: [u8; 20] = hasher.finalize().into();
        if hash == self.hash {
            Ok(())
        } else {
            Err(Error::InvalidHash { idx: self.idx, expected: self.hash, actual: hash })
        }
    }
}