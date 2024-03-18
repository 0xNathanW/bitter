use std::{
    net::{Ipv4Addr, SocketAddr}, 
    path::PathBuf, 
    time::Duration
};
use url::Url;

#[derive(Debug, Clone)]
pub struct Config {

    pub client_id: [u8; 20],

    pub dir: PathBuf,

    pub listen_address: SocketAddr,

    pub custom_trackers: Vec<Url>,

    pub announce_interval: Duration,

    pub min_max_peers: (u32, u32),

}

const DEFAULT_CLIENT_ID: [u8; 20] = *b"-RS0133-73b3b0b0b0b0";

impl Default for Config {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID,
            dir: PathBuf::from("downloads"),
            announce_interval: Duration::from_secs(1800),
            custom_trackers: Vec::new(),
            listen_address: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 6881),
            min_max_peers: (5, 100),
        }
    }
}