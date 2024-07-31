use tokio::sync::mpsc;

mod config;
mod metainfo;
mod info;
mod torrent;
mod tracker;
mod client;
mod p2p;
mod disk;
mod block;
mod picker;
mod de;
pub mod stats;

// Most commonly used block size - 16KB.
const BLOCK_SIZE: usize = 0x4000;

type Bitfield = bitvec::vec::BitVec<u8, bitvec::order::Msb0>;

// 20 byte SHA1 info hash.
pub type ID = [u8; 20];

// Messages the users of the client expect to recieve.
pub enum UserCommand {

    // Sent when a torrent has finished.
    TorrentFinished {
        id: ID,
    },

    // Sent every second with the current stats of a torrent.
    TorrentStats {
        id: ID,
        stats: stats::TorrentStats,
    },
}

type UserTx = mpsc::UnboundedSender<UserCommand>;
pub type UserRx = mpsc::UnboundedReceiver<UserCommand>;

use client::{ClientCommand, ClientTx};

// Re-exports
pub use config::Config;
pub use client::{Result, ClientError};
pub use p2p::state::{SessionState, ConnState};
pub use metainfo::MetaInfo;
pub use torrent::{TorrentError, TorrentState};

pub fn start_client(config: Option<Config>) -> (Handle, UserRx) {
    let (user_tx, user_rx) = mpsc::unbounded_channel();
    let (mut client, client_tx) = client::Client::new(config.unwrap_or_default(), user_tx);
    let client_handle = tokio::spawn(async move { 
        if let Err(e) = client.run().await {
            tracing::error!("client runtime error:  {:?}", e);
        }
    });
    (
        Handle {
            client_tx,
            client_handle,
        },
        user_rx
    )
}

// Handle returned to the user to interact with the client.
pub struct Handle {

    client_tx: ClientTx,
    
    client_handle: tokio::task::JoinHandle<()>,
    
}

impl Handle {
    
        pub fn new_torrent(&self, metainfo: MetaInfo) -> Result<()> {
            self.client_tx.send(ClientCommand::NewTorrent(metainfo))?;
            Ok(())
        }

        pub async fn remove_torrent(&self, id: ID) -> Result<()> {
            self.client_tx.send(ClientCommand::RemoveTorrent(id))?;
            Ok(())
        }

        pub async fn shutdown(self) -> Result<()> {
            self.client_tx.send(ClientCommand::Shutdown).ok();
            self.client_handle.await.map_err(|_| ClientError::ClientPanic)?;
            Ok(())
        }

}