use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use crate::{client::{ClientTx, CommandToClient}, TorrentID};
use super::*;

pub struct Disk {
    
    // Currently active torrents.
    torrents: HashMap<TorrentID, RwLock<torrent::Torrent>>,
    
    // Commands to the disk task.
    disk_rx: DiskRx,

    // Send commands to the client task.
    client_tx: ClientTx,

}

impl Disk {

    pub fn new(client_tx: ClientTx) -> (Self, DiskTx) {
        let (disk_tx, disk_rx) = mpsc::unbounded_channel();
        (
            Disk {
                torrents: HashMap::new(),
                disk_rx,
                client_tx,
            },
            disk_tx
        )
    }

    pub async fn run(&mut self) -> Result<()> {

        while let Some(cmd) = self.disk_rx.recv().await {
            match cmd {

                CommandToDisk::NewTorrent { 
                    id, 
                    info,
                    piece_hashes,
                    torrent_tx,
                } => {
                    let msg = if self.torrents.contains_key(&id) {
                        CommandToClient::TorrentAllocation(Err(AllocationError::DuplicateTorrent))
                    } else {
                        match torrent::Torrent::new(info, piece_hashes, torrent_tx) {
                            Ok(torrent) => {
                                // Allocate the new torrent.
                                self.torrents.insert(id, RwLock::new(torrent));
                                CommandToClient::TorrentAllocation(Ok(id))
                            },
                            Err(e) => {
                                CommandToClient::TorrentAllocation(Err(e))
                            },
                        }
                    };
                    self.client_tx.send(msg)?;
                },

                // TODO: handle write and read errors differently?
                CommandToDisk::WriteBlock { id, block } => {
                    self.torrents
                        .get(&id)
                        .ok_or_else(|| DiskError::TorrentNotFound(hex::encode(id)))?
                        .write()
                        .await
                        .write_block(block)?;
                },

                CommandToDisk::ReadBlock { id, block, tx } => {
                    self.torrents
                        .get(&id)
                        .ok_or_else(|| DiskError::TorrentNotFound(hex::encode(id)))?
                        .read()
                        .await
                        .read_block(block, tx)?;
                },

                CommandToDisk::Shutdown => {
                    break;
                },

            }
        }

        Ok(())
    }
}