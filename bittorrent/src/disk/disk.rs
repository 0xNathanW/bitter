use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use crate::ID;
use super::*;

pub struct Disk {
    
    // Currently active torrents.
    torrents: HashMap<ID, RwLock<torrent::Torrent>>,
    
    // Commands to the disk task.
    disk_rx: DiskRx,

}

impl Disk {

    pub fn new() -> (Self, DiskTx) {
            let (disk_tx, disk_rx) = mpsc::unbounded_channel();
            (
                Disk {
                torrents: HashMap::new(),
                disk_rx,
            },
            disk_tx
        )
    }

    pub async fn run(&mut self) {

        while let Some(cmd) = self.disk_rx.recv().await {
            match cmd {

                DiskCommand::NewTorrent { 
                    id,
                    info,
                    piece_hashes,
                    files,
                    dir,
                    torrent_tx,
                    tx,
                } => {

                    let msg = if self.torrents.contains_key(&id) {
                        Err(AllocationError::DuplicateTorrent)
                    } else {
                        match torrent::Torrent::new(files, dir, piece_hashes, info, torrent_tx) {
                            
                            Ok(torrent) => {
                                // Allocate the new torrent.
                                // Maybe run this in a separate task, particularly the checking?
                                let bf = torrent.check_existing_files();
                                self.torrents.insert(id, RwLock::new(torrent));
                                Ok(bf)
                            },
                            
                            Err(e) => Err(e),
                        }
                    };
                
                    let _ = tx.send(msg);
                },

                DiskCommand::RemoveTorrent(id) => {
                    if let Some(torrent) = self.torrents.remove(&id) {
                        // Wait for write lock to wait for pending writes/reads, then drop.
                        let _ = torrent.write().await;
                    } else {
                        tracing::warn!("attempted to remove non-existent torrent: {}", hex::encode(id));
                    }
                },

                DiskCommand::WriteBlock { id, block } => {
                    if let Some(torrent) = self.torrents.get(&id) {
                        torrent
                            .write()
                            .await
                            .write_block(block);
                    } else {
                        tracing::warn!("torrent {} not found on disk", hex::encode(id));
                        continue;
                    }
                },

                DiskCommand::ReadBlock { id, block, tx } => {
                    if let Some(torrent) = self.torrents.get(&id) {
                        torrent
                            .read()
                            .await
                            .read_block(block, tx);
                    } else {
                        tracing::warn!("torrent {} not found on disk", hex::encode(id));
                        continue;
                    }
                },

                DiskCommand::Shutdown => {
                    break;
                },

            }
        }
    }
}