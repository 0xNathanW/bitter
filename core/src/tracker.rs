use std::{io, time};
use serde_derive::{Serialize, Deserialize};
use reqwest::{Client, Url};
use super::torrent;

const PORT : u16 = 6881;

enum TrackerError {
    IoError(io::Error),
    ParseError,
}

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
            client:     Client::new(),
            params:     RequestParams::new(torrent),
            interval:   None,
            epoch:      time::Instant::now(),
        }
    }

    pub async fn get_peers(&self) -> String {
        let url = Url::parse_with_params(&self.announce, &self.params).unwrap();
        
        "test".to_string()
    }
}

#[derive(Serialize)]
pub struct RequestParams {
    // Hash of info dict.
    info_hash:  String,
    // Urlencoded 20-byte string used as a unique ID for the client.
    peer_id:    String,
    // Port number.
    port:       u16,
    // Total amount uploaded.
    uploaded:   u64,
    // Total bytes downloaded.
    downloaded: u64,
    // Total bytes left to download.
    left:       u64,
    // peers list is replaced by a peers string with 6 bytes per peer.
    compact:    u8,
}

impl RequestParams {

    pub fn new(torrent: &torrent::Torrent) -> RequestParams {
        RequestParams {
            info_hash: hex::encode(torrent.info_hash()),
            peer_id: "-RS0133-".to_string(),
            port: PORT,
            uploaded: 0,
            downloaded: 0,
            left: torrent.size(),
            compact: 1,
        }
    }
}

#[derive(Deserialize)]
pub struct TrackerResponse {
    // 
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;
    use hex_literal::hex;

    fn url_parsing() {
        let announce = "http://tracker.example.com:6969/announce";
        let params = RequestParams {
            info_hash:  hex!("0123456789abcdef0123456789abcdef01234567").to_string(),
            peer_id:    hex!("0123456789abcdef0123456789abcdef01234567").to_string(),
            port:       PORT,
            uploaded:   0,
            downloaded: 0,
            left:       0,
            compact:    1,
            event:      "started".to_string(),
        };

        let url = Url::parse_with_params(announce, &params).unwrap();
        assert_eq!(url.as_str(), 
        "http://tracker.example.com:6969/announce?info_hash=%01%23Eg%89%AB%CD%EF%01%23Eg%89%AB%CD%EF%01%23Eg&peer_id=%01%23Eg%89%AB%CD%EF%01%23Eg%89%AB%CD%EF%01%23Eg&port=6881&uploaded=0&downloaded=0&left=0&compact=1&event=started"
    );

    }

}