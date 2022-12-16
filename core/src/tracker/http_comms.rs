use serde_bytes::ByteBuf;
use serde_derive::{Deserialize, Serialize};
use urlencoding::encode_binary;

use crate::torrent;
use super::PORT;
use super::peer_parse::{ParsePeers, PeerInfo};

// Request params are serialized into a query string.
#[derive(Serialize)]
pub struct RequestParams {
    announce:   String,
    // Hash of info dict.
    info_hash:  [u8; 20],
    // Urlencoded 20-byte string used as a unique ID for the client.
    peer_id:    [u8; 20],
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
            announce:   torrent.announce().to_string(),
            info_hash:  torrent.info_hash().clone(),
            peer_id:    b"-RS0133-73b3b0b0b0b0".to_owned(),
            port:       PORT,
            uploaded:   0,
            downloaded: 0,
            left:       torrent.size(),
            compact:    1,
        }
    }

    // Update request params after the first request with new values.
    pub fn refresh_params(&mut self, uploaded: u64, downloaded: u64, left: u64) {
        self.uploaded = uploaded;
        self.downloaded = downloaded;
        self.left = left;
    }

    pub fn build_url(&self, id: &Option<String>) -> String {
        let url = format!(
            "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
            self.announce,
            encode_binary(&self.info_hash),
            encode_binary(&self.peer_id),
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            self.compact,
        );

        if let Some(id) = id {
            format!("{}&trackerid={}", url, id)
        } else {
            url   
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct TrackerResponse<P: ParsePeers> {
    // If present, then no other keys may be present. 
    // The value is a human-readable error message as to why the request failed (string).
    #[serde(rename = "failure reason")]
    pub failure_reason: Option<ByteBuf>,

    // Option code for the failure reason (integer).
    #[serde(rename = "failure code")]
    pub failure_code: Option<u64>,

    // (new, optional) Similar to failure reason, but the response still gets processed normally. 
    // The warning message is shown just like an error.
    #[serde(rename = "warning message")]
    pub warning_message: Option<ByteBuf>,

    // Interval in seconds that the client should wait between sending regular requests to the tracker
    pub interval: Option<u64>,

    // Minimum announce interval. If present clients must not reannounce more frequently than this.
    #[serde(rename = "min interval")]
    pub min_interval: Option<u64>,

    // A string that the client should send back on its next announcements.
    #[serde(rename = "tracker id")]
    pub tracker_id: Option<ByteBuf>,

    // Number of peers with the entire file, i.e. seeders (integer)
    pub complete: Option<u64>,

    // Number of non-seeder peers, aka "leechers" (integer)
    pub incomplete: Option<u64>,

    // (dictionary model)
    pub peers: Option<P>,
}

impl<P: ParsePeers> TrackerResponse<P> {
    pub fn peers(&self) -> Option<Vec<PeerInfo>> {
        self.peers.as_ref().map(|p| p.parse_peers())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::peer_parse::BinaryModel;
    use std::net::{SocketAddrV4, Ipv4Addr};
    use reqwest::Client;
    use sha1::{Digest, Sha1};
    use urlencoding::{encode, encode_binary};

    #[test]
    fn test_parse_request() {
        let announce = "http://tracker.example.com/announce";
        let info_hash: [u8; 20] = hex::decode("d8e8fca2dc0f896fd7cb4cb0031ba249b33e213b").unwrap().try_into().unwrap();

        let params = RequestParams {
            info_hash,
            announce:   announce.to_string(),
            peer_id:    b"-RS0133-73b3b0b0b0b0".to_owned(),
            port:       PORT,
            uploaded:   0,
            downloaded: 0,
            left:       0,
            compact:    1,
        };

        let url: String = Client::new()
            .get(params.build_url(&None))
            .build()
            .unwrap()
            .url()
            .clone()
            .into();

        assert_eq!(url,
        concat!(
            "http://tracker.example.com/announce?",
            "info_hash=%D8%E8%FC%A2%DC%0F%89o%D7%CBL%B0%03%1B%A2I%B3%3E%21%3B",
            "&peer_id=-RS0133-73b3b0b0b0b0",
            "&port=6881",
            "&uploaded=0",
            "&downloaded=0",
            "&left=0",
            "&compact=1",
        ));   
    }

    #[test]
    fn test_parse_response_binary() {
        let s = "64383a636f6d706c65746569396531303a696e636f6d706c657465693165383a696e74657276616c69313830306531323a6d696e20696e74657276616c693138303065353a706565727336303a52454d051ae1ca2f2a2ec00884937726decc61759ab8138851ab05e8f6bb5062f69770469247493ad4d005879f2ec8d54237ce44ea6043db8806c8d565";
        let raw = hex::decode(s).unwrap();

        let response: TrackerResponse<BinaryModel> = bencode::decode_bytes(&raw).unwrap();        
        assert_eq!(response.interval, Some(1800));
        assert_eq!(response.min_interval, Some(1800));
        assert_eq!(response.complete, Some(9));
        assert_eq!(response.incomplete, Some(1));

        let peers = response.peers.unwrap().parse_peers();
        assert!(peers.contains(&PeerInfo {
            addr: SocketAddrV4::new(Ipv4Addr::new(97, 117, 154, 184), 5000),
            id:   None,
        }));

        assert!(peers.contains(&PeerInfo {
            addr: SocketAddrV4::new(Ipv4Addr::new(5, 135, 159, 46), 51413),
            id:   None,
        }));
    }
}