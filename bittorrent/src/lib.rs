use tokio::sync::mpsc;

mod config;
mod metainfo;
mod store;
mod torrent;
mod client;
mod tracker;
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
pub type TorrentID = [u8; 20];

// Messages the users of the client expect to recieve.
pub enum UserCommand {

    // Sent when a torrent has finished.
    TorrentResult {
        id: TorrentID,
        result: torrent::Result<()>,
    },

    // Sent every second with the current stats of a torrent.
    TorrentStats {
        id: TorrentID,
        stats: stats::TorrentStats,
    },
}

type UserTx = mpsc::UnboundedSender<UserCommand>;
pub type UserRx = mpsc::UnboundedReceiver<UserCommand>;

// Re-exports
pub use client::{Result, ClientHandle, ClientError};
pub use p2p::state::{SessionState, ConnState};
pub use config::Config;
pub use metainfo::MetaInfo;
pub use torrent::{TorrentError, TorrentState};

pub fn start_client(config: Option<Config>) -> (ClientHandle, UserRx) {
    let (user_tx, user_rx) = mpsc::unbounded_channel();
    let (mut client, client_tx) = client::Client::new(config.unwrap_or_default(), user_tx);
    tracing::info!("starting client");
    let handle = tokio::spawn(async move { client.run().await });
    (ClientHandle::new(client_tx, handle), user_rx)
}