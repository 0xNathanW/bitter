use std::collections::HashMap;
use tokio::{sync::mpsc, task};
use crate::{
    config::{ClientConfig, TorrentConfig}, 
    fs::{self, CommandToDisk}, 
    metainfo::MetaInfo, 
    store::StoreInfo, 
    torrent::{self, Torrent, TorrentParams}, 
    CommandToUser, 
    TorrentID, 
    UserTx,
};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
        
        // #[error(transparent)]
        // TorrentError(#[from] torrent::TorrentError),
    
        #[error(transparent)]
        DiskError(#[from] fs::DiskError),
    
        #[error("channel error: {0}")]
        ChannelError(String),

}

impl<T> From<mpsc::error::SendError<T>> for ClientError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        ClientError::ChannelError(e.to_string())
    }
}

pub enum CommandToClient {

    NewTorrent {
        id:         TorrentID,
        metainfo:   MetaInfo,
        config:     Option<TorrentConfig>,
    },

    TorrentAllocation(std::result::Result<TorrentID, fs::AllocationError>),

    DiskFailure(fs::DiskError),

    Shutdown,

}

pub type Result<T> = std::result::Result<T, ClientError>;
pub type ClientRx = mpsc::UnboundedReceiver<CommandToClient>;
pub type ClientTx = mpsc::UnboundedSender<CommandToClient>;

pub struct ClientHandle {

    client_tx: ClientTx,

    handle: Option<task::JoinHandle<Result<()>>>

}

// These are the methods that the user will use to interact with the client.
impl ClientHandle {

    pub fn new(client_tx: ClientTx, handle: task::JoinHandle<Result<()>>) -> Self {
        Self { client_tx, handle: Some(handle) }
    }

    pub fn new_torrent(&self, metainfo: MetaInfo, config: Option<TorrentConfig>) -> Result<TorrentID> {
        let id = metainfo.info_hash();
        self.client_tx.send(CommandToClient::NewTorrent { id, metainfo, config })?;
        Ok(id)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.client_tx.send(CommandToClient::Shutdown)?;
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

    torrent_tx: torrent::TorrentTx,

    handle: Option<tokio::task::JoinHandle<torrent::Result<()>>>

}

pub struct Client {

    torrents: HashMap<TorrentID, TorrentHandle>,

    client_rx: ClientRx,

    user_tx: UserTx,

    disk_tx: fs::DiskTx,

    disk_handle: Option<task::JoinHandle<fs::Result<()>>>,

    config: ClientConfig,

}

impl Client {
    
    pub fn new(config: ClientConfig, user_tx: UserTx) -> Result<(Self, ClientTx)> {
        let (client_tx, client_rx) = mpsc::unbounded_channel();
        let (disk_handle, disk_tx) = fs::spawn_disk(client_tx.clone())?;
        Ok((
            Client {
                torrents: HashMap::new(),
                client_rx,
                user_tx,
                disk_tx,
                disk_handle: Some(disk_handle),
                config,
            },
            client_tx,
        ))
    }

    pub async fn run(&mut self) -> Result<()> {

        while let Some(cmd) = self.client_rx.recv().await {
            match cmd {
             
                CommandToClient::NewTorrent { 
                    id,
                    metainfo,
                    config,
                } => self.new_torrent(id, metainfo, config).await?,

                CommandToClient::TorrentAllocation(id) => {
                    match id {
                        Ok(id) => tracing::info!("torrent {} allocated", hex::encode(id)),
                        Err(e) => {
                            tracing::error!("torrent allocation error: {}", e);
                            self.user_tx.send(CommandToUser::TorrentError(e.to_string()))?;
                        }
                    }
                },

                // Client can't continue if the disk fails.
                CommandToClient::DiskFailure(e) => {
                    self.shutdown().await?;
                    return Err(ClientError::DiskError(e))
                },

                CommandToClient::Shutdown => return self.shutdown().await,

            }
        }

        Ok(())
    }

    async fn new_torrent(&mut self, id: TorrentID, metainfo: MetaInfo, config: Option<TorrentConfig>) -> Result<()> {
        
        let torrent_config = config.unwrap_or_default();
        let info = StoreInfo::new(&metainfo, torrent_config.output_dir.clone());
        let piece_hashes = metainfo.piece_hashes();
        
        let (mut torrent, torrent_tx) = Torrent::new(TorrentParams {
            id,
            client_id: self.config.client_id,
            metainfo,
            config: torrent_config,
            disk_tx: self.disk_tx.clone(),
            user_tx: self.user_tx.clone(),
        });

        self.disk_tx.send(CommandToDisk::NewTorrent {
            id,
            info,
            piece_hashes,
            torrent_tx: torrent_tx.clone(),
        })?;

        let handle = task::spawn(async move { torrent.start().await });
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
            torrent.torrent_tx.send(torrent::CommandToTorrent::Shutdown).ok();
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

        self.disk_tx.send(CommandToDisk::Shutdown)?;
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