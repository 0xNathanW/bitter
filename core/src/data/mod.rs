use thiserror::Error;

mod piece;
mod block;
mod queue;
mod bitfield;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Index mismatch: expected {0}, got {1}")]
    IndexMismatch(u32, u32),

    #[error("Recieved block has invalid bounds: {0}")]
    BlockInvalidBounds(String),

    #[error("Recieved piece #{idx} with invalid hash: expected {expected:?}, got {actual:?}")]
    InvalidHash {
        idx: u32,
        expected: [u8; 20],
        actual: [u8; 20],
    },
}