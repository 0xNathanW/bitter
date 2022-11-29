use std::net::Ipv4Addr;
use serde_derive::Deserialize;

// Standardised return type for different peer models.
pub struct PeerInfo {
    id: String,
    ip: Ipv4Addr,
    port: u16,
}

// The tracker can either return a dictionary model or a compacted string.
// This is based on the value of the "compact" parameter.
// However, even if we request a compacted string, the tracker can still return a dictionary model.
pub trait ParsePeers {
    fn parse_peers(&self) -> Vec<PeerInfo>;
}


// --- (dictionary model) The value is a list of dictionaries --- //

type DictModel = Vec<PeerItem>;

#[derive(Deserialize)]
struct PeerItem {
    // Peer's self-selected ID (string)
    peer_id: String,
    // The IP address of the peer (string)
    ip: String,
    // The port number of the peer (integer)
    port: u16,
}

impl ParsePeers for DictModel {

    fn parse_peers(&self) -> Vec<PeerInfo> {
        self.iter().map(|p| PeerInfo {
            id: p.peer_id.clone(),
            ip: p.ip.parse().unwrap(),
            port: p.port,
        }).collect()
    }

}

// -- (binary model) The value is a string whose length is a multiple of 6 --- //
// The first 4 bytes are the IP address and the last 2 bytes are the port number.
// All in network (big-endian) byte order.
type BinaryModel = String;

impl ParsePeers for BinaryModel {

    fn parse_peers(&self) -> Vec<PeerInfo> {
        let mut peers = Vec::new();
        let mut bytes = self.as_bytes();
        for raw in bytes.chunks(6) {
            let ip = Ipv4Addr::new(raw[0], raw[1], raw[2], raw[3]);
            let port = u16::from_be_bytes([raw[4], raw[5]]);
            peers.push(PeerInfo {
                id: "".to_string(),
                ip,
                port,
            });
        }
        peers 
    }

}

#[cfg(test)]
mod tests {

    #[test]
    fn test_parse_peers_binary() {

    }

}