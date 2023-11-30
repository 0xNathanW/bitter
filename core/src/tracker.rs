use std::{
    net::{SocketAddr, IpAddr, Ipv4Addr}, 
    time::{Instant, Duration},
};
use bytes::Buf;
use reqwest::{Client, Url};
use serde::de;
use serde_derive::Deserialize;

// In cases where the tracker doesn't give us a min interval.
const DEFAULT_MIN_ANNOUNCE_INTERVAL: u64 = 60; // seconds

pub type Result<T> = std::result::Result<T, TrackerError>;

#[derive(thiserror::Error, Debug)]
pub enum TrackerError {
    #[error("tracker request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("error parsing tracker url: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("error deserializing tracker response: {0}")]
    BencodeError(#[from]bencode::Error),

    #[error("tracker Error: code({code:?}), {msg:?}")]
    TrackerError {
        msg: String,
        code: Option<u64>,
    },

    #[error("Error: {0}")]
    Custom(String),
}

#[derive(Debug)]
pub struct Tracker {
    // HTTP client.
    client: Client,

    // Tracker announce url.
    pub url: Url,

    // Tracker id, if sent by response.
    pub tracker_id: Option<String>,

    // Last time we sent an announce request.
    pub last_announce: Option<Instant>,

    // Interval for next announce request.
    pub interval: Option<Duration>,

    // Minimum interval for next announce request.
    pub min_interval: Option<Duration>,
}

impl Tracker {

    pub fn new(url: Url) -> Tracker {
        Tracker {
            client: Client::new(),
            url,
            tracker_id: None,
            last_announce: None,
            interval: None,
            min_interval: None,
        }
    }

    // Sends announce to tracker.
    pub async fn send_announce(&self, params: AnnounceParams) -> Result<TrackerResponse> {

        let mut url = format!(
            "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1",
            self.url.as_str(),
            urlencoding::encode_binary(&params.info_hash),
            urlencoding::encode_binary(&params.peer_id),
            params.port,
            params.uploaded,
            params.downloaded,
            params.left,
        );
        if let Some(event) = params.event {
            url.push_str(&format!("&event={}", event));
        }
        if let Some(num_peers) = params.num_want {
            url.push_str(&format!("&numwant={}", num_peers));
        }
        if let Some(tracker_id) = params.tracker_id {
            url.push_str(&format!("&tracker_id={}", tracker_id));
        }

        let raw_resp = self.client
            .get(url)
            .send()
            .await?
            .bytes()
            .await?;
        let resp: TrackerResponse = bencode::decode_bytes(&raw_resp)?;
        
        Ok(resp)
    }

    // Returns true if time since last announce is greater than interval.
    pub fn should_announce(&self, time: Instant) -> bool {
        
        if let Some(last_announce) = self.last_announce {
            time.duration_since(last_announce) 
            >= self.interval.unwrap_or(Duration::from_secs(DEFAULT_MIN_ANNOUNCE_INTERVAL))
        
        // If we haven't announced yet.
        } else {
            true
        }
    }

    // Returns true if time since last announce is greater than min interval.
    pub fn can_announce(&self, time: Instant) -> bool {

        if let Some(last_announce) = self.last_announce {
            time.duration_since(last_announce) 
            >= self.min_interval.unwrap_or(Duration::from_secs(DEFAULT_MIN_ANNOUNCE_INTERVAL))
        
        // If we haven't announced yet.
        } else {
            true
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Event {
    Started,
    Stopped,
    Completed,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Started => write!(f, "started"),
            Event::Stopped => write!(f, "stopped"),
            Event::Completed => write!(f, "completed"),
        }
    }
}

// Announce params are serialized into a query string.
#[derive(Debug)]
pub struct AnnounceParams {
    
    // Hash of info dict.
    pub info_hash:  [u8; 20],
    
    // Urlencoded 20-byte string used as a unique ID for the client.
    pub peer_id:    [u8; 20],
    
    // Port number.
    pub port:       u16,
    
    // Total amount uploaded.
    pub uploaded:   u64,
    
    // Total bytes downloaded.
    pub downloaded: u64,
    
    // Total bytes left to download.
    pub left:       u64,
    
    // If specified, must be one of started, completed, stopped, (or empty which is the same as not being specified). 
    // If not specified, then this request is one performed at regular intervals.
    pub event:     Option<Event>,
    
    // Number of peers that the client would like to receive from the tracker.
    pub num_want: Option<usize>,

    // If a previous announce contained a tracker id, it should be set here.
    pub tracker_id: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct TrackerResponse {

    // If present, then no other keys may be present. 
    // The value is a human-readable error message as to why the request failed (string).
    #[serde(rename = "failure reason")]
    pub failure_reason: Option<String>,

    // (new, optional) Similar to failure reason, but the response still gets processed normally. 
    // The warning message is shown just like an error.
    #[serde(rename = "warning message")]
    pub warning_message: Option<String>,

    // Interval in seconds that the client should wait between sending regular requests to the tracker
    pub interval: Option<u64>,

    // Minimum announce interval. If present clients must not reannounce more frequently than this.
    #[serde(rename = "min interval")]
    pub min_interval: Option<u64>,

    // A string that the client should send back on its next announcements.
    #[serde(rename = "tracker id")]
    pub tracker_id: Option<String>,

    // Number of peers with the entire file, i.e. seeders (integer)
    pub complete: Option<u64>,

    // Number of non-seeder peers, aka "leechers" (integer)
    pub incomplete: Option<u64>,

    // (dictionary model)
    #[serde(default)]
    #[serde(deserialize_with = "peer_derserialize")]
    pub peers: Vec<SocketAddr>,
}

// The tracker can either return a dictionary model or a compacted string.
// This is based on the value of the "compact" parameter.
// However, even if we request a compacted string, the tracker can still return a dictionary model.
fn peer_derserialize<'de, D>(deserializer: D) -> std::result::Result<Vec<SocketAddr>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct PeerVisitor;

    impl<'de> de::Visitor<'de> for PeerVisitor {

        type Value = Vec<SocketAddr>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string of bytes or a list of dictionaries")
        }

        // String model.
        // The first 4 bytes are the IP address and the last 2 bytes are the port number.
        // All in network (big-endian) byte order.
        fn visit_bytes<E>(self, mut v: &[u8]) -> std::result::Result<Self::Value, E>
        where
            E: de::Error, 
        {   
            
            if v.len() % 6 != 0 {
                return Err(
                    TrackerError::Custom("Peer string length not a multiple of 6".to_string()
                )).map_err(E::custom);
            }

            let num_peers = v.len() / 6;
            let mut peers = Vec::with_capacity(num_peers);
            for _ in 0..num_peers {
                peers.push(
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::from(v.get_u32())),
                        v.get_u16(),
                    )
                )
            }
            
            Ok(peers)
        }

        // Dictionary model.
        // The dictionary model is a list of dictionaries, each with the keys "ip" and "port".
        fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>, 
        {
            #[derive(Deserialize)]
            struct PeerItem {
                ip: String,
                port: u16,
            }

            let mut peers = Vec::new();
            while let Some(peer) = seq.next_element::<PeerItem>()? {
                let ip = match peer.ip.parse::<u32>() {
                    Ok(ip) => SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), peer.port),
                    Err(_) => continue,
                };
                peers.push(ip);
            }

            Ok(peers)
        }
    }

    deserializer.deserialize_any(PeerVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_binary() {
        let s = "64383a636f6d706c65746569396531303a696e636f6d706c657465693165383a696e74657276616c69313830306531323a6d696e20696e74657276616c693138303065353a706565727336303a52454d051ae1ca2f2a2ec00884937726decc61759ab8138851ab05e8f6bb5062f69770469247493ad4d005879f2ec8d54237ce44ea6043db8806c8d565";
        let raw = hex::decode(s).unwrap();

        let response: TrackerResponse = bencode::decode_bytes(&raw).unwrap();        
        assert_eq!(response.interval, Some(1800));
        assert_eq!(response.min_interval, Some(1800));
        assert_eq!(response.complete, Some(9));
        assert_eq!(response.incomplete, Some(1));

        assert!(response.peers.contains(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(97, 117, 154, 184)), 5000)));

        assert!(response.peers.contains(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(5, 135, 159, 46)), 51413)));
    }
}