use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use crate::{client::{ClientTx, ClientCommand}, TorrentID};
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

    #[tracing::instrument(skip(self))]
    pub async fn run(&mut self) -> Result<()> {

        while let Some(cmd) = self.disk_rx.recv().await {
            match cmd {

                DiskCommand::NewTorrent { 
                    id,
                    info,
                    piece_hashes,
                    files,
                    dir,
                    torrent_tx,
                } => {
                    let msg = if self.torrents.contains_key(&id) {
                        // Torrent already exists in the disk task.
                        ClientCommand::TorrentAllocation{
                            id,
                            res: Err(AllocationError::DuplicateTorrent),
                        }
                    } else {
                        match torrent::Torrent::new(files, dir, piece_hashes, info, torrent_tx) {
                            Ok(torrent) => {
                                // Allocate the new torrent.
                                // Maybe run this in a separate task, particularly the checking?
                                let bf = torrent.check_existing_files();
                                self.torrents.insert(id, RwLock::new(torrent));
                                ClientCommand::TorrentAllocation { id, res: Ok(bf)}
                            },
                            Err(e) => ClientCommand::TorrentAllocation { id, res: Err(e)},
                        }
                    };
                    self.client_tx.send(msg)?;
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
                            .read_block(block, tx)?;
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
        Ok(())
    }
}