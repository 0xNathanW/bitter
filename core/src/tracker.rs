use std::time;
use serde::de::DeserializeOwned;
use serde_derive::{Serialize, Deserialize};
use reqwest::Client;
use super::torrent;

const PORT : u16 = 6881;

type Result<T> = std::result::Result<T, reqwest::Error>;

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

    pub async fn get_peers(&self) -> Result<String> {
        let raw_resp = self.client.get(&self.announce)
            .query(&self.params)
            .send()
            .await?
            .text()
            .await?;

        let resp = bencode::decode_str(&raw_resp)?;
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

    fn new(torrent: &torrent::Torrent) -> RequestParams {
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
}

#[derive(Deserialize)]
struct TrackerResponse<P> {
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

#[derive(Deserialize)]
struct Peer {
    // Peer's self-selected ID (string)
    peer_id: String,
    // The IP address of the peer (string)
    ip: String,
    // The port number of the peer (integer)
    port: u16,
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