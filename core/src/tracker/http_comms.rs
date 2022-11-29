use serde_derive::{Deserialize, Serialize};

use crate::torrent;
use super::PORT;
use super::peer_parse::ParsePeers;

// Request params are serialized into a query string.
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
            info_hash:  hex::encode(torrent.info_hash()),
            peer_id:    "-RS0133-".to_string(),
            port:       PORT,
            uploaded:   0,
            downloaded: 0,
            left:       torrent.size(),
            compact:    1,
        }
    }

    pub fn refresh_params(&mut self, uploaded: u64, downloaded: u64, left: u64) {
        self.uploaded = uploaded;
        self.downloaded = downloaded;
        self.left = left;
    }
}

#[derive(Deserialize)]
pub struct TrackerResponse<P: ParsePeers> {
    // If present, then no other keys may be present. 
    // The value is a human-readable error message as to why the request failed (string).
    #[serde(rename = "failure reason")]
    failure_reason: Option<String>,

    // (new, optional) Similar to failure reason, but the response still gets processed normally. 
    // The warning message is shown just like an error.
    #[serde(rename = "warning message")]
    warning_message: Option<String>,

    // Interval in seconds that the client should wait between sending regular requests to the tracker
    interval: Option<u64>,

    // Minimum announce interval. If present clients must not reannounce more frequently than this.
    #[serde(rename = "min interval")]
    min_interval: Option<u64>,

    // A string that the client should send back on its next announcements.
    #[serde(rename = "tracker id")]
    tracker_id: Option<String>,

    // Number of peers with the entire file, i.e. seeders (integer)
    complete: Option<u64>,

    // Number of non-seeder peers, aka "leechers" (integer)
    incomplete: Option<u64>,

    // (dictionary model)
    peers: Option<P>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[test]
    fn url_parsing() {
        let announce = "http://tracker.example.com:6969/announce";
        let params = RequestParams {
            info_hash:  "0123456789abcdef0123456789abcdef01234567".to_string(),
            peer_id:    "0123456789abcdef0123456789abcdef01234567".to_string(),
            port:       PORT,
            uploaded:   0,
            downloaded: 0,
            left:       0,
            compact:    1,
        };

        let url = Client::new()
            .get(announce)
            .query(&params)
            .build()
            .unwrap()
            .url()
            .clone();
        println!("url: {}", url);

        assert_eq!(url.as_str(), 
        "http://tracker.example.com:6969/announce?info_hash=%01%23Eg%89%AB%CD%EF%01%23Eg%89%AB%CD%EF%01%23Eg&peer_id=%01%23Eg%89%AB%CD%EF%01%23Eg%89%AB%CD%EF%01%23Eg&port=6881&uploaded=0&downloaded=0&left=0&compact=1"
    );

    }
}