use std::net::{SocketAddrV4, Ipv4Addr};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use super::message::Message;
use super::{Result, Error};
use super::bitfield::Bitfield;
use super::handshake::*;

pub struct Peer {
    
    id:        Option<String>,
    addr:      SocketAddrV4,
    stream:    Option<tokio::net::TcpStream>,
    bitfield:  Bitfield,

    pub choked:             bool,
    pub interested:         bool,
    pub peer_choking:       bool,
    pub peer_interested:    bool,

    display_chan: Option<mpsc::Sender<String>>,
}

impl Default for Peer {
    fn default() -> Self {
        Self {
            id:                 None,
            addr:               SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0),
            stream:             None,
            bitfield:           Bitfield::new(0),
            choked:             true,
            interested:         false,
            peer_choking:       true,
            peer_interested:    false,
            display_chan:       None,
        }
    }
}

impl Peer {

    pub fn new(id: Option<String>, addr: SocketAddrV4) -> Self {
        Self {
            id,
            addr,
            ..Default::default()
        }
    }

    // Involves everything up to the point where we start trading pieces.
    pub async fn connect(
        &mut self, 
        info_hash: [u8; 20], 
        id: [u8; 20],
        display_chan: Option<mpsc::Sender<String>>,
    ) -> Result<()> {
        let stream = TcpStream::connect(self.addr).await?;
        self.stream = Some(stream);
        self.display_chan = display_chan;
        self.exchange_handshake(info_hash, id).await?;
        self.build_bitfield().await?;
        Ok(())
    }

    pub async fn disconnect(&mut self) {
        if let Some(stream) = &mut self.stream {
            stream.shutdown().await.ok();
        }
    }

    async fn exchange_handshake(&mut self, info_hash: [u8; 20], id: [u8; 20]) -> Result<()> {
        
        let msg = handshake(info_hash, id);
        if let Some(stream) = &mut self.stream {
            stream.write_all(&msg).await?;
        } else {
            return Err(Error::NoStream);
        }

        let mut buf = vec![0; 68];
        if let Some(stream) = &mut self.stream {
            stream.read_exact(&mut buf).await?;
        } else {
            return Err(Error::NoStream);
        }
        verify_handshake(buf, info_hash)?;
        
        Ok(())
    }

    pub async fn send(&mut self, msg: Message) -> Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.write_all(&msg.encode()).await?;
        } else {
            return Err(Error::NoStream);
        }
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Message> {
        if let Some(stream) = &mut self.stream {
            
            let mut buf = vec![0; 4];
            stream.read_exact(&mut buf).await?;
            let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            
            let mut buf = vec![0; len];
            stream.read_exact(&mut buf).await?;
            Ok(Message::decode(&buf)?)
        
        } else {
            Err(Error::NoStream)
        }
    }

    pub fn set_bitfield(&mut self, bitfield: Bitfield) {
        self.bitfield = bitfield;
    }

    pub fn has_piece(&self, idx: u32) -> bool {
        self.bitfield.has_piece(idx)
    }

    pub fn set_piece(&mut self, idx: u32) {
        self.bitfield.set_piece(idx);
    }

    pub fn new_port(&mut self, port: u16) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::PeerInfo;
    use std::net::{SocketAddrV4, Ipv4Addr};
    use rand::Rng;

    #[tokio::test] 
    async fn test_peer () {
        let info_hash = [189, 0, 237, 28, 241, 142, 87, 90, 92, 184, 41, 212, 52, 155, 206, 237, 52, 215, 104, 51];
        let id = rand::thread_rng().gen::<[u8; 20]>();
        // Abitrary real peer.
        let info = PeerInfo {
            id: None,
            addr: SocketAddrV4::new(Ipv4Addr::new(81, 171, 5, 232), 63163),
        };

        let mut peer = Peer::new(None, info.addr);
        peer.connect(info_hash, id, None).await.unwrap();
    }
}