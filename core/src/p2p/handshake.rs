use std::{io::Cursor, fmt::Debug};
use bytes::{BufMut, Buf};
use tokio_util::codec::{Encoder, Decoder};
use super::PeerError;

pub const PROTOCOL: [u8; 19] = *b"BitTorrent protocol";

pub struct Handshake {
    pub protocol:   [u8; 19],
    pub reserved:   [u8; 8],
    pub info_hash:  [u8; 20],
    pub peer_id:    [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20]) -> Self {
        Self {
            protocol:   PROTOCOL,
            reserved:   [0; 8],
            info_hash,
            peer_id:    *b"-RS0133-73b3b0b0b0b0",
        }
    }
}

pub struct HandshakeCodec;

impl Encoder<Handshake> for HandshakeCodec {

    type Error = PeerError;

    fn encode(&mut self, item: Handshake, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        dst.put_u8(19);
        dst.extend_from_slice(&item.protocol);
        dst.extend_from_slice(&item.reserved);
        dst.extend_from_slice(&item.info_hash);
        dst.extend_from_slice(&item.peer_id);
        Ok(())
    }
}

impl Decoder for HandshakeCodec {

    type Item = Handshake;
    type Error = PeerError;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        
        if src.is_empty() {
            return Ok(None);
        }

        let mut peeker = Cursor::new(&src[..]);
        let prt_len = peeker.get_u8();
        if prt_len != 19 {
            return Err(PeerError::IncorrectProtocol);
        }

        if src.remaining() != 67 {
            src.advance(1);
        } else {
            return Ok(None)
        }
        
        // Protocol
        let mut protocol = [0; 19];
        src.copy_to_slice(&mut protocol);

        // Reserved
        let mut reserved = [0; 8];
        src.copy_to_slice(&mut reserved);

        // Info hash
        let mut info_hash = [0; 20];
        src.copy_to_slice(&mut info_hash);

        // Peer id
        let mut peer_id = [0; 20];
        src.copy_to_slice(&mut peer_id);

        Ok(Some(Handshake {
            protocol,
            reserved,
            info_hash,
            peer_id,
        }))
    }
}

impl Debug for Handshake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handshake")
            .field("protocol", &String::from_utf8_lossy(&self.protocol))
            .field("reserved", &self.reserved)
            .field("info_hash", &String::from_utf8_lossy(&self.info_hash))
            .field("peer_id", &String::from_utf8_lossy(&self.peer_id))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_decoding() {
        let mut src = bytes::BytesMut::new();
        src.put_u8(19);
        src.extend_from_slice(b"BitTorrent protocol");
        src.extend_from_slice(&[0; 8]);
        src.extend_from_slice(&[0; 20]);
        src.extend_from_slice(&[0; 20]);

        let mut decoder = HandshakeCodec;
        let handshake = decoder.decode(&mut src).unwrap().unwrap();
        assert_eq!(handshake.protocol, *b"BitTorrent protocol");
        assert_eq!(handshake.reserved, [0; 8]);
        assert_eq!(handshake.info_hash, [0; 20]);
        assert_eq!(handshake.peer_id, [0; 20]);
    }

    #[test]
    fn test_protocol_error() {
        let mut src = bytes::BytesMut::new();
        src.put_u8(18);
        src.extend_from_slice(b"BitTorrent protocol wrong");
        src.extend_from_slice(&[0; 8]);
        src.extend_from_slice(&[0; 20]);
        src.extend_from_slice(&[0; 20]);

        let mut decoder = HandshakeCodec;
        let handshake = decoder.decode(&mut src);
        assert!(handshake.is_err());
    }
}