use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, time::{Duration, Instant}};
use bytes::Buf;
use url::Url;
use serde::de;
use serde_derive::Deserialize;
use super::{AnnounceParams, Result, Tracker, TrackerError, DEFAULT_MIN_ANNOUNCE_INTERVAL};

pub struct HttpTracker {

    client: reqwest::Client,

    url: Url,

    id: Option<String>,

    last_announce: Option<Instant>,

    interval: Option<Duration>,

    min_interval: Option<Duration>,

}

impl HttpTracker {
    pub fn new(url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            id: None,
            last_announce: None,
            interval: None,
            min_interval: None,
        }
    }
}

#[async_trait::async_trait]
impl Tracker for HttpTracker {
    
    async fn announce(&mut self, params: AnnounceParams) -> Result<Vec<SocketAddr>> {

        let mut url = format!(
            "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1",
            self.url.as_str(),
            urlencoding::encode_binary(&params.info_hash),
            urlencoding::encode_binary(&params.client_id),
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
        if let Some(tracker_id) = &self.id {
            url.push_str(&format!("&tracker_id={}", tracker_id));
        }
        tracing::debug!("announce url: {}", url);

        let raw_resp = self.client
            .get(url)
            .send()
            .await?
            .bytes()
            .await?;

        let resp: HttpResponse = bencode::decode_bytes(&raw_resp)?;
        tracing::debug!("announce response: {:#?}", resp);
        
        if let Some(failure) = resp.failure_reason {
            return Err(TrackerError::ResponseError(failure));
        }
        if let Some(warning) = resp.warning_message {
            tracing::warn!("warning: {}", warning);
        }

        if let Some(interval) = resp.interval {
            self.interval = Some(Duration::from_secs(interval));
        }
        if let Some(min_interval) = resp.min_interval {
            self.min_interval = Some(Duration::from_secs(min_interval));
        }
        if let Some(tracker_id) = resp.tracker_id {
            self.id = Some(tracker_id);
        }

        self.last_announce = Some(Instant::now());
        Ok(resp.peers)
    }

    fn can_announce(&self, time: Instant) -> bool {

        if let Some(last_announce) = self.last_announce {
            time.duration_since(last_announce) 
            >= self.min_interval.unwrap_or(Duration::from_secs(DEFAULT_MIN_ANNOUNCE_INTERVAL))

        } else {
            true
        }
    }

    fn should_announce(&self, time: Instant) -> bool {
            
        if let Some(last_announce) = self.last_announce {
            time.duration_since(last_announce) 
            >= self.interval.unwrap_or(Duration::from_secs(DEFAULT_MIN_ANNOUNCE_INTERVAL))

        } else {
            true
        }
    } 
}

#[derive(Deserialize, Debug, Default)]
pub struct HttpResponse {

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
                return Err(E::custom("peer string not multiple of 6"));
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
        let response: HttpResponse = bencode::decode_bytes(&hex::decode(s).unwrap()).unwrap();        
        assert_eq!(response.interval, Some(1800));
        assert_eq!(response.min_interval, Some(1800));
        assert_eq!(response.complete, Some(9));
        assert_eq!(response.incomplete, Some(1));
        assert!(response.peers.contains(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(97, 117, 154, 184)), 5000)));
        assert!(response.peers.contains(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(5, 135, 159, 46)), 51413)));
    }
}