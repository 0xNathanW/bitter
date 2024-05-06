use std::collections::HashMap;
use tokio::{sync::mpsc, task::JoinHandle};
use crate::{
    config::Config, 
    disk::{self, AllocationError, DiskCommand, DiskError, DiskTx}, 
    metainfo::MetaInfo, 
    store::TorrentInfo, 
    torrent::{self, Torrent, TorrentParams, TorrentTx}, 
    Bitfield, 
    TorrentID, 
    UserCommand, 
    UserTx
};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    
        #[error(transparent)]
        DiskError(#[from] DiskError),
    
        #[error("client channel error: {0}")]
        ChannelError(String),

}

impl<T> From<mpsc::error::SendError<T>> for ClientError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        ClientError::ChannelError(e.to_string())
    }
}

pub enum ClientCommand {

    NewTorrent(MetaInfo),

    TorrentAllocation {
        id: TorrentID,
        res: std::result::Result<Bitfield, AllocationError>,
    },

    DiskFailure(DiskError),

    Shutdown,

}

pub type Result<T> = std::result::Result<T, ClientError>;
pub type ClientRx = mpsc::UnboundedReceiver<ClientCommand>;
pub type ClientTx = mpsc::UnboundedSender<ClientCommand>;

// Handle returned to the user to interact with the client.
pub struct ClientHandle {

    client_tx: ClientTx,

    handle: Option<JoinHandle<Result<()>>>

}

// These are the methods that the user will use to interact with the client.
impl ClientHandle {

    pub fn new(client_tx: ClientTx, handle: JoinHandle<Result<()>>) -> Self {
        Self { client_tx, handle: Some(handle) }
    }

    pub fn new_torrent(&self, metainfo: MetaInfo) -> Result<()> {
        self.client_tx.send(ClientCommand::NewTorrent(metainfo))?;
        Ok(())
    }

    // TODO: pause, resume, remove torrent.

    pub async fn shutdown(&mut self) -> Result<()> {
        self.client_tx.send(ClientCommand::Shutdown)?;
        if let Err(e) = self
            .handle
            .take()
            .expect("client task already shut down")
            .await
            .expect("client task panicked")
        {
            return Err(e);
        }
        Ok(())
    }

}

struct TorrentHandle {

    torrent_tx: TorrentTx,

    handle: Option<JoinHandle<torrent::Result<()>>>

}

pub struct Client {

    torrents: HashMap<TorrentID, TorrentHandle>,

    client_rx: ClientRx,

    user_tx: UserTx,

    disk_tx: DiskTx,

    disk_handle: Option<JoinHandle<disk::Result<()>>>,

    config: Config,

}

impl Client {
    
    pub fn new(config: Config, user_tx: UserTx) -> (Self, ClientTx) {
        let (client_tx, client_rx) = mpsc::unbounded_channel();
        let (disk_handle, disk_tx) = disk::spawn_disk(client_tx.clone());
        (
            Client {
                torrents: HashMap::new(),
                client_rx,
                user_tx,
                disk_tx,
                disk_handle: Some(disk_handle),
                config,
            },
            client_tx,
        )
    }

    pub async fn run(&mut self) -> Result<()> {

        while let Some(cmd) = self.client_rx.recv().await {
            match cmd {
                
                ClientCommand::NewTorrent(metainfo) => self.new_torrent(metainfo).await?,

                ClientCommand::TorrentAllocation { id, res } => {
                    match res {
                        Ok(bf) => {
                            tracing::info!("torrent {} allocated", hex::encode(id));
                            if let Some(t) = self.torrents.get_mut(&id) {
                                t.torrent_tx.send(torrent::TorrentCommand::Bitfield(bf))?;
                            }
                        },
                        Err(e) => {
                            tracing::error!("torrent allocation error: {}", e);
                            if let Some(t) = self.torrents.get_mut(&id) {
                                t.torrent_tx.send(torrent::TorrentCommand::Shutdown).ok();
                            }
                            self.user_tx.send(UserCommand::TorrentResult { id, result: Err(e.into()) })?;
                        }
                    }
                },

                // Client can't continue if the disk fails.
                ClientCommand::DiskFailure(e) => {
                    self.shutdown().await?;
                    return Err(ClientError::DiskError(e))
                },

                ClientCommand::Shutdown => return self.shutdown().await,

            }
        }

        Ok(())
    }

    async fn new_torrent(&mut self, metainfo: MetaInfo) -> Result<()> {
        
        let id = metainfo.info_hash();
        let info: TorrentInfo = TorrentInfo::new(&metainfo);
        let piece_hashes = metainfo.piece_hashes();

        let (mut torrent, torrent_tx) = Torrent::new(TorrentParams {
            id,
            info: info.clone(),
            info_hash: metainfo.info_hash(),
            client_id: self.config.client_id,
            trackers: metainfo.trackers(),
            config: self.config.clone(),
            disk_tx: self.disk_tx.clone(),
            user_tx: self.user_tx.clone(),
        });
        
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
        self.disk_tx.send(DiskCommand::NewTorrent {
            id,
            info,
            piece_hashes,
            files,
            dir,
            torrent_tx: torrent_tx.clone(),
        })?;

        let handle = tokio::task::spawn(async move { torrent.start().await });
        self.torrents.insert(
            id,
            TorrentHandle {
                torrent_tx,
                handle: Some(handle),
            },
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        
        for torrent in self.torrents.values_mut() {
            // Some torrents may have already been shut down so don't return err.
            torrent.torrent_tx.send(torrent::TorrentCommand::Shutdown).ok();
        }

        for torrent in self.torrents.values_mut() {
            if let Err(e) = torrent
                .handle
                .take()
                .expect("torrent task already taken")
                .await
                .expect("torrent task panicked")
            {
                tracing::error!("torrent task error: {}", e);
            }
        }

        self.disk_tx.send(DiskCommand::Shutdown)?;
        if let Err(e) = self
            .disk_handle
            .take()
            .expect("disk task already taken")
            .await
            .expect("disk task panicked")
        {
            tracing::error!("disk task error: {}", e);
        }

        Ok(())
    }

}