use std::net::{SocketAddrV4, Ipv4Addr};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        }
    }
}

pub async fn connect(ip: SocketAddrV4, id: Option<String>, info_hash: [u8; 20]) -> Result<Peer> {

    let stream = TcpStream::connect(ip).await?;
    Ok(Peer {
        stream: Some(stream),
        ..Default::default()
    })

}

impl Peer {

    pub async fn exchange_handshake(&mut self, info_hash: [u8; 20], id: [u8; 20]) -> Result<()> {
        
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
}