use std::net::SocketAddrV4;

use super::bitfield::Bitfield;

pub struct Peer {
    
    id:        Option<String>,
    addr:      SocketAddrV4,
    stream:    Option<tokio::net::TcpStream>,
    bitfield:  Bitfield,

    pub choked:             bool,
    pub interested:         bool,
    pub peer_choking:       bool,
    pub peer_interested:    bool,


}