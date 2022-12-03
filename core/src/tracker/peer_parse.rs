use std::net::{SocketAddrV4, Ipv4Addr};
use serde_bytes::ByteBuf;
use serde_derive::Deserialize;

// Standardised return type for different peer models.
#[derive(Debug, PartialEq)]
pub struct PeerInfo {
    pub addr: SocketAddrV4,
    pub id:   Option<String>,
}

// The tracker can either return a dictionary model or a compacted string.
// This is based on the value of the "compact" parameter.
// However, even if we request a compacted string, the tracker can still return a dictionary model.
pub trait ParsePeers {
    fn parse_peers(&self) -> Vec<PeerInfo>;
}

// --- (dictionary model) The value is a list of dictionaries --- //
pub type DictModel = Vec<PeerItem>;

#[derive(Deserialize)]
pub struct PeerItem {
    // Peer's self-selected ID (string)
    peer_id: Option<String>,
    // The IP address of the peer (string)
    ip: String,
    // The port number of the peer (integer)
    port: u16,
}

impl ParsePeers for DictModel {
    fn parse_peers(&self) -> Vec<PeerInfo> {

        let mut peers = Vec::new();
        for peer in self {
            
            let addr = match peer.ip.parse::<u32>() {
                Ok(ip) => SocketAddrV4::new(Ipv4Addr::from(ip), peer.port),
                Err(_) => continue,
            };
            
            peers.push(PeerInfo {
                addr,
                id: peer.peer_id.clone(),
            });
        }   
        peers
    }
}

// -- (binary model) The value is a string whose length is a multiple of 6 --- //
// The first 4 bytes are the IP address and the last 2 bytes are the port number.
// All in network (big-endian) byte order.
pub type BinaryModel = ByteBuf;

impl ParsePeers for BinaryModel {
    fn parse_peers(&self) -> Vec<PeerInfo> {

        let mut peers = Vec::new();
        for raw in self.chunks_exact(6) {
        
            let ip = SocketAddrV4::new(
                u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]).into(),
                u16::from_be_bytes([raw[4], raw[5]]),
            );
        
            peers.push(PeerInfo { addr: ip, id: None });
        }
        peers 
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn test_parse_peers_binary() {
        let raw = vec![
            82, 69, 77, 5, 26, 225, 
            97, 117, 134, 73, 19, 136, 
            81, 171, 5, 232, 246, 187, 
            80, 98, 246, 151, 112, 70, 
            146, 71, 73, 58, 212, 208, 
            5, 135, 159, 46, 200, 213, 
            66, 55, 206, 68, 234, 96, 
            67, 219, 136, 6, 200, 213
        ];        
        let peers = ByteBuf::from(raw).parse_peers();
        
        assert_eq!(peers.len(), 8);
        
        assert!(peers.contains(&PeerInfo {
            addr: SocketAddrV4::new(Ipv4Addr::new(82, 69, 77, 5), 6881),
            id:   None,
        }));
        
        assert!(peers.contains(&PeerInfo {
            addr: SocketAddrV4::new(Ipv4Addr::new(97, 117, 134, 73), 5000),
            id:   None,
        }));

        assert!(peers.contains(&PeerInfo {
            addr: SocketAddrV4::new(Ipv4Addr::new(5, 135, 159, 46), 51413),
            id:   None,
        }));
    }
}