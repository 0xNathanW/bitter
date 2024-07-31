use tokio::{sync::{mpsc, oneshot}, task::{self, JoinHandle}};
use tracing::Instrument;
use crate::{
    block::{Block, BlockRequest}, info::TorrentInfo, metainfo, p2p::PeerTx, torrent::TorrentTx, Bitfield, ID
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

type Result<T> = std::result::Result<T, DiskError>;
pub type DiskTx = mpsc::UnboundedSender<DiskCommand>;
type DiskRx = mpsc::UnboundedReceiver<DiskCommand>;

// TODO: command for removing a torrent from disk?
pub enum DiskCommand {

    NewTorrent {
        id: ID,
        info: TorrentInfo,
        piece_hashes: Vec<ID>,
        files: Vec<metainfo::File>,
        dir: std::path::PathBuf,
        torrent_tx: TorrentTx,
        // Sends the bitfield to the torrent task.
        tx: oneshot::Sender<std::result::Result<Bitfield, AllocationError>>,
    },

    RemoveTorrent(ID),

    // From peers sending blocks, write block data to disk.
    WriteBlock {
        id: ID,
        block: Block,
    },

    // From peers requesting blocks, read data from disk
    // and send read data through provided channel.
    ReadBlock {
        id: ID,
        block: BlockRequest,
        tx: PeerTx,
    },

    #[allow(dead_code)]
    Shutdown,

}

pub fn start_disk() -> (JoinHandle<()>, DiskTx) {
    let (mut disk, disk_tx) = disk::Disk::new();
    let handle = task::spawn(async move {
        disk.run().await
    }.instrument(tracing::info_span!("disk")));
    (handle, disk_tx)
}
