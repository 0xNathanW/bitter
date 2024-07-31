use std::collections::HashMap;
use tokio::sync::mpsc;
use crate::{
    config::Config, 
    disk::{start_disk, DiskCommand, DiskTx},
    metainfo::MetaInfo,
    info::TorrentInfo,
    torrent::{self, TorrentHandle, TorrentParams},
    ID,
    UserTx,
};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {

        #[error("client has been unexpectedly dropped")]
        ClientDropped(#[from] mpsc::error::SendError<ClientCommand>),

        #[error("client panicked")]
        ClientPanic,
        
        #[error("disk task panicked")]
        DiskFailure(#[from] mpsc::error::SendError<DiskCommand>),
}

pub enum ClientCommand {

    NewTorrent(MetaInfo),

    RemoveTorrent(ID),

    Shutdown,

}

pub type Result<T> = std::result::Result<T, ClientError>;
pub type ClientRx = mpsc::UnboundedReceiver<ClientCommand>;
pub type ClientTx = mpsc::UnboundedSender<ClientCommand>;

pub struct Client {

    client_rx: ClientRx,

    torrents: HashMap<ID, TorrentHandle>,

    user_tx: UserTx,

    config: Config,

    // Last used listening port.
    // Incremented by 1 for each new torrent.
    current_port: u16,

}

impl Client {
    
    pub fn new(config: Config, user_tx: UserTx) -> (Self, ClientTx) {
        
        let (client_tx, client_rx) = mpsc::unbounded_channel();
        let current_port = config.listen_port_start;
        
        (
            Client {
                torrents: HashMap::new(),
                client_rx,
                user_tx,
                config,
                current_port,
            },
            client_tx,
        )
    }

    pub async fn run(&mut self) -> Result<()> {
        
        // Start the disk task.
        let (_, disk_tx) = start_disk();

        while let Some(cmd) = self.client_rx.recv().await {
            match cmd {
                
                ClientCommand::NewTorrent(metainfo) => self.new_torrent(metainfo, &disk_tx).await?,

                ClientCommand::RemoveTorrent(id) => {
                    if let Some(torrent) = self.torrents.remove(&id) {
                        let _ = torrent.torrent_tx.send(torrent::TorrentCommand::Shutdown);
                        disk_tx.send(DiskCommand::RemoveTorrent(id))?;
                    } else {
                        tracing::warn!("attempted to remove non-existent torrent: {}", hex::encode(id));
                    }
                }

                ClientCommand::Shutdown => return Ok(self.shutdown().await),

            }
        }

        Ok(())
    }

    async fn new_torrent(&mut self, metainfo: MetaInfo, disk_tx: &DiskTx) -> Result<()> {
        
        let info_hash = metainfo.info_hash();
        let info: TorrentInfo = TorrentInfo::new(&metainfo);
        let piece_hashes = metainfo.piece_hashes();
        let (tx, rx) = tokio::sync::oneshot::channel();

        let torrent_handle = TorrentHandle::start_torrent(
            TorrentParams {
                info: info.clone(),
                info_hash,
                client_id: self.config.client_id,
                tracker_urls: metainfo.tracker_urls(),
                config: self.config.clone(),
                disk_tx: disk_tx.clone(),
                user_tx: self.user_tx.clone(),
                listen_port: self.current_port,
            },
            rx,
        );

        // If the torrent is multi file, create a directory for it.
        let dir = if metainfo.is_multi_file() {
            self.config.dir.join(metainfo.info.name.clone())
        } else {
            self.config.dir.clone()
        };
        // If the torrent is single file, create a single element vector. 
        let files = if let Some(files) = metainfo.info.files {
            files
        } else {
            vec![crate::metainfo::File {
                path: vec![metainfo.info.name.clone()],
                length: metainfo.total_len(),
                md5sum: metainfo.info.md5sum,
            }]
        };
        // Tell the disk to allocate the torrent.
        disk_tx.send(DiskCommand::NewTorrent {
            id: info_hash,
            info,
            piece_hashes,
            files,
            dir,
            torrent_tx: torrent_handle.torrent_tx.clone(),
            tx,
        })?;
        // Increment the port for the next torrent.
        self.current_port += 1;

        self.torrents.insert(info_hash, torrent_handle);
        Ok(())
    }

    async fn shutdown(&mut self) {

        for torrent in self.torrents.values_mut() {
            // Some torrents may have already been shut down so don't return err.
            torrent.torrent_tx.send(torrent::TorrentCommand::Shutdown).ok();
        }

        for (id, torrent) in self.torrents.drain() {
            if let Err(e) = torrent.handle.await {
                tracing::error!("torrent {} pacicked: {}", hex::encode(id), e);
            }
        }
    }

}