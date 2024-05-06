use tokio::{sync::mpsc, task};
use crate::{
    block::{Block, BlockRequest}, 
    client::ClientTx, 
    metainfo,
    p2p::PeerTx,
    store::TorrentInfo,
    torrent::TorrentTx,
    TorrentID
};

mod piece;
mod disk;
mod torrent;
#[cfg(test)]
mod tests;

#[derive(thiserror::Error, Debug)]
pub enum DiskError {

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("io error: expected {expected} bytes, got {actual}")]
    IoSizeError {
        expected: usize,
        actual: usize,
    },

    // Usually relating to poisoned file locks.
    #[error("sync error: {0}")]
    SyncError(String),

    #[error("channel error: {0}")]
    ChannelError(String),

    #[error("torrent {0:?} not found")]
    TorrentNotFound(TorrentID),
}

// Errors related to allocating a new torrent to disk.
#[derive(thiserror::Error, Debug)]
pub enum AllocationError {
    
    #[error("torrent already exists in disk task")]
    DuplicateTorrent,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

}

impl<T> From<std::sync::PoisonError<T>> for DiskError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        DiskError::SyncError(e.to_string())
    }
}

impl<T> From<mpsc::error::SendError<T>> for DiskError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        DiskError::ChannelError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DiskError>;
pub type DiskTx = mpsc::UnboundedSender<DiskCommand>;
pub type DiskRx = mpsc::UnboundedReceiver<DiskCommand>;

// TODO: command for removing a torrent from disk?
pub enum DiskCommand {

    // Allocate a new torrent to disk.
    // Quite large but only sent once.
    NewTorrent {
        // To identify the torrent.
        id: TorrentID,
        // Info for reads and writes.
        info: TorrentInfo,
        // Piece hashes for verification.
        piece_hashes: Vec<[u8; 20]>,
        // Torrent files.
        files: Vec<metainfo::File>,
        // Output directory for the torrent.
        dir: std::path::PathBuf,
        // To send commands to the torrent task.
        torrent_tx: TorrentTx,
    },

    // From peers sending blocks, write block data to disk.
    WriteBlock {
        id: TorrentID,
        block: Block,
    },

    // From peers requesting blocks, read data from disk
    // and send read data through provided channel.
    ReadBlock {
        id: TorrentID,
        block: BlockRequest,
        tx: PeerTx,
    },

    // Shutdown the disk task.
    Shutdown,

}

pub fn spawn_disk(client_tx: ClientTx) -> (task::JoinHandle<Result<()>>, DiskTx) {
    tracing::info!("starting disk task");
    let (mut disk, disk_tx) = disk::Disk::new(client_tx);
    let handle = task::spawn(async move { disk.run().await });
    (handle, disk_tx)
}

