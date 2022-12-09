use thiserror::Error;

const PORT: u16 = 6881;

mod peer_parse;
mod http_comms;
pub mod tracker;
pub use tracker::Tracker;
pub use peer_parse::PeerInfo;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Error deserializing tracker response: {0}")]
    BencodeError(#[from]bencode::Error),

    #[error("Tracker Error:
        code({code:?}), 
        {msg:?}
    ")]
    TrackerError {
        msg: String,
        code: Option<u64>,
    },
}

#[cfg(test)]
mod tests {
    use std::net::{SocketAddrV4, Ipv4Addr};
    use std::path::Path;
    use crate::torrent;
    use super::*;
    use super::peer_parse::PeerInfo;

    #[tokio::test]
    async fn test_tracker() {
        let path = Path::new("../test_torrents/test_single_file.torrent");
        let torrent = torrent::Torrent::new(&path).expect("Failed to create torrent");
        println!("{:?}", torrent.info_hash());
        let mut tracker = tracker::Tracker::new(&torrent);
        let fut = tracker.request_peers();
        let (peers, _active, _inactive) = fut.await.unwrap();
        println!("Peers: {:#?}", peers);
    }
}