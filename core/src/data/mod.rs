use thiserror::Error;

mod piece;
mod block;
mod queue;
mod bitfield;

pub use bitfield::Bitfield;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Piece(#[from] piece::Error),

    #[error(transparent)]
    Block(#[from] block::Error),
}