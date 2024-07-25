use std::{path::PathBuf, time::Duration};
use url::Url;

use crate::ID;

#[derive(Debug, Clone)]
pub struct Config {

    pub client_id: ID,

    pub dir: PathBuf,

    pub listen_port_start: u16,

    pub custom_trackers: Vec<Url>,

    pub announce_interval: Duration,

    pub max_peers: usize,

}

const DEFAULT_CLIENT_ID: ID = *b"-RS0133-73b3b0b0b0b0";

impl Default for Config {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID,
            dir: PathBuf::from("downloads"),
            announce_interval: Duration::from_secs(1800),
            custom_trackers: Vec::new(),
            listen_port_start: 49152,  // IANA registered ephemeral ports.
            max_peers: 50,
        }
    }
}