use std::time;
use reqwest::Client;

use crate::torrent;
use super::http_comms::{RequestParams, TrackerResponse};

pub struct Tracker {
    announce:   String,
    // A string the client should send to the tracker in its next request.
    id:         String,
    // http client.
    client:     Client,
    // Get request query parameters.
    params:     RequestParams,
    // How long client should wait before sending next request.
    interval:   Option<time::Duration>,
    // Time of last request.
    epoch:      time::Instant,
}

impl Tracker {

    pub fn new(torrent: &torrent::Torrent) -> Tracker {
        Tracker {
            announce:   torrent.announce().to_string(),
            id:         "".to_string(),
            client:     Client::new(),
            params:     RequestParams::new(torrent),
            interval:   None,
            epoch:      time::Instant::now(),
        }
    }

    pub fn refresh_params(&mut self, uploaded: u64, downloaded: u64, left: u64) {
        self.params.refresh_params(uploaded, downloaded, left);
    }

    // pub async fn request_peers(&self) -> Result<PeersInfo> {
    //     let raw_resp = self.client.get(&self.announce)
    //         .query(&self.params)
    //         .send()
    //         .await?
    //         .text()
    //         .await?;

    //     let resp = bencode::decode_str(&raw_resp)?;
    //     "test".to_string()
    // }
}