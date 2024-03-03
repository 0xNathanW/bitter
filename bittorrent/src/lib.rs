use tokio::sync::mpsc;

mod config;
mod metainfo;
mod store;
mod torrent;
mod stats;
mod client;
mod tracker;
mod p2p;
mod fs;
mod block;
mod picker;
mod de;

const BLOCK_SIZE: usize = 0x4000;

type Bitfield = bitvec::vec::BitVec<u8, bitvec::order::Msb0>;

pub type TorrentID = [u8; 20];

// Messages the users of the client expect to recieve.
pub enum CommandToUser {

    // Sent when a torrent has finished downloading.
    TorrentComplete(TorrentID),

    // Sent when a torrent has failed.
    TorrentError(String),

    TorrentStats {
        id: TorrentID,
        stats: stats::TorrentStats,
    },

}

type UserTx = mpsc::UnboundedSender<CommandToUser>;
pub type UserRx = mpsc::UnboundedReceiver<CommandToUser>;

pub use client::{Result, ClientHandle, ClientError};
pub use config::{ClientConfig, TorrentConfig};
pub use metainfo::MetaInfo;
pub use stats::TorrentStats;

pub fn start_client(config: Option<ClientConfig>) -> Result<(ClientHandle, UserRx)> {
    let (user_tx, user_rx) = mpsc::unbounded_channel();
    let (mut client, client_tx) = client::Client::new(config.unwrap_or_default(), user_tx)?;
    let handle = tokio::spawn(async move { client.run().await });
    Ok((ClientHandle::new(client_tx, handle), user_rx))
}