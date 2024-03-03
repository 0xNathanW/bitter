use tokio::{sync::mpsc, task};
use crate::{
    block::{Block, BlockInfo}, 
    client::ClientTx,
    p2p::PeerTx,
    store::StoreInfo, 
    torrent::TorrentTx, 
    TorrentID,
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

    #[error("sync error: {0}")]
    SyncError(String),

    #[error("channel error: {0}")]
    ChannelError(String),

    #[error("torrent info hash {0} not found")]
    TorrentNotFound(String),

    #[error("torrent allocation error: {0}")]
    AllocationError(#[from] AllocationError),
}

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
pub type DiskTx = mpsc::UnboundedSender<CommandToDisk>;
pub type DiskRx = mpsc::UnboundedReceiver<CommandToDisk>;

// TODO: command for removing a torrent from disk?
pub enum CommandToDisk {

    // Allocate a new torrent to disk.
    NewTorrent {
        // To identify the torrent.
        id: TorrentID,
        // Info for reads and writes.
        info: StoreInfo,
        // Piece hashes for verification.
        piece_hashes: Vec<[u8; 20]>,
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
        block: BlockInfo,
        tx: PeerTx,
    },

    // Shutdown the disk task.
    Shutdown,

}

pub fn spawn_disk(client_tx: ClientTx) -> Result<(task::JoinHandle<Result<()>>, DiskTx)> {
    tracing::info!("starting disk task");
    let (mut disk, disk_tx) = disk::Disk::new(client_tx);
    let handle = task::spawn(async move { disk.run().await });
    Ok((handle, disk_tx))
}

