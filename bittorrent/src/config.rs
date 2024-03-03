use std::{
    net::{Ipv4Addr, SocketAddr}, 
    path::PathBuf, 
    time::Duration
};

#[derive(Debug, Clone)]
pub struct TorrentConfig {
    
    pub output_dir: PathBuf,    

    pub listen_address: SocketAddr,

    pub announce_interval: Duration,

    pub min_max_peers: (u32, u32),

}

impl Default for TorrentConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("downloads"),
            listen_address: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 6881),
            announce_interval: Duration::from_secs(1800),
            min_max_peers: (5, 100),
        }
    }
}

pub struct ClientConfig {

    pub client_id: [u8; 20],

    // Global max peers.

    // Global upload slots.

}

const DEFAULT_CLIENT_ID: [u8; 20] = *b"-RS0133-73b3b0b0b0b0";

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID,
        }
    }
}