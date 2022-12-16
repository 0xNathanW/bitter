use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid piece index for block: expected {0}, got {1}")]
    IndexMismatch(u32, u32),

    #[error("Invalid block bounds")]
    InvalidBounds,
}

pub struct Block {
    pub piece_idx: u32,
    pub offset: u32,
    pub data: Vec<u8>,
}

