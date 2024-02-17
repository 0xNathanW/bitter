use bytes::{BufMut, Buf, BytesMut};
use tokio_util::codec::{Encoder, Decoder};
use crate::{block, Bitfield};
use super::PeerError;

#[cfg_attr(test, derive(Debug, Clone, PartialEq, Eq))]
pub enum Message {
    
    // The keep alive message advises peers not to close the connection, 
    // even if they haven’t received a message in some time.
    KeepAlive,
    
    // A choke message tells a peer that no further requests will be satisfied.
    Choke,
    
    // Conversely unchoke signifies that requests from the peer will be served.
    Unchoke,
    
    // Notifies a peer that the client is interested in making requests for blocks.
    Interested,

    // Notifies a peer the client is no longer interested in requesting blocks.
    NotInterested,
    
    // Tells a peer that the client has a piece, referenced by the piece index.
    Have { idx: u32 },

    // The bitfield message is a short form method of communicating to a peer what pieces 
    // a client has usually sent after the handshake has been completed.
    Bitfield(Bitfield),

    // When a client wants to request data, they reference the index of the piece, the index 
    // of the start of the block within the piece, ank the length of tle block (usually 16KB).
    Request(block::BlockInfo),

    // Clients senk blocks in tle piece message, referencing piece index and block offset.
    Block(block::BlockData),

    // The cancel message is sent to cancel a request for a block.
    Cancel(block::BlockInfo),

    // The port message is sent to inform the peer of the port number that the client is listening on.
    Port { port: u32 },
}

pub struct MessageCodec;

impl Encoder<Message> for MessageCodec {

    type Error = PeerError;

    fn encode(&mut self, msg: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match msg {

            // [0, 0, 0, 0]
            Message::KeepAlive => dst.put_u32(0),

            // [0, 0, 0, 1, 0]
            Message::Choke => {
                dst.put_u32(1);
                dst.put_u8(0);
            },

            // [0, 0, 0, 1, 1]
            Message::Unchoke => {
                dst.put_u32(1);
                dst.put_u8(1);
            },

            // [0, 0, 0, 1, 2]
            Message::Interested => {
                dst.put_u32(1);
                dst.put_u8(2);
            },

            // [0, 0, 0, 1, 3]
            Message::NotInterested => {
                dst.put_u32(1);
                dst.put_u8(3);
            },

            // have: <len=0005><id=4><piece index>
            Message::Have { idx } => {
                dst.put_u32(5);
                dst.put_u8(4);
                dst.put_u32(idx);
            },

            // bitfield: <len=0001+X><id=5><bitfield>
            Message::Bitfield(bitfield) => {
                dst.put_u32(1 + (bitfield.len() / 8) as u32);
                dst.put_u8(5);
                dst.extend_from_slice(&bitfield.as_raw_slice());
            },

            // request: <len=0013><id=6><index><begin><length>
            Message::Request(block) => {
                dst.put_u32(13);
                dst.put_u8(6);
                dst.put_u32(block.piece_idx as u32);
                dst.put_u32(block.offset as u32);
                dst.put_u32(block.len as u32);
            },

            // piece: <len=0009+X><id=7><index><begin><block>
            Message::Block(block) => {
                dst.put_u32(9 + block.data.len() as u32);
                dst.put_u8(7);
                dst.put_u32(block.piece_idx as u32);
                dst.put_u32(block.offset as u32);
                dst.extend_from_slice(&block.data);
            },

            // cancel: <len=0013><id=8><index><begin><length>
            Message::Cancel(block) => {
                dst.put_u32(13);
                dst.put_u8(8);
                dst.put_u32(block.piece_idx as u32);
                dst.put_u32(block.offset as u32);
                dst.put_u32(block.len as u32);
            },

            // port: <len=0003><id=9><listen-port>
            Message::Port { port } => {
                dst.put_u32(3);
                dst.put_u8(9);
                dst.put_u32(port);
            },
        }

        Ok(())
    }
}

impl Decoder for MessageCodec {
    
    type Item = Message;
    type Error = PeerError;
    
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        
        // Can't read message length.
        if src.remaining() < 4 { return Ok(None); }

        let mut peeker = std::io::Cursor::new(&src);
        let msg_len: usize = peeker.get_u32() as usize;
        peeker.set_position(0);

        if src.remaining() >= 4 + msg_len {
            src.advance(4);
            if msg_len == 0 { return Ok(Some(Message::KeepAlive)); }
        } else {
            // Haven't recieved all of message.
            return Ok(None);
        }

        let msg = match src.get_u8() {
            0 => Message::Choke,
            1 => Message::Unchoke,
            2 => Message::Interested,
            3 => Message::NotInterested,
            4 => Message::Have { idx: src.get_u32() },
            5 => {
                let mut bitfield = vec![0; msg_len - 1];
                src.copy_to_slice(&mut bitfield);
                Message::Bitfield(Bitfield::from_vec(bitfield))
            },
            6 => {
                let piece_idx = src.get_u32() as usize;
                let offset = src.get_u32() as usize;
                let len = src.get_u32() as usize;
                Message::Request(block::BlockInfo { piece_idx, offset, len })
            },
            7 => {
                let piece_idx = src.get_u32() as usize;
                let offset = src.get_u32() as usize;
                let mut data = vec![0; msg_len - 9];
                src.copy_to_slice(&mut data);
                Message::Block(block::BlockData { piece_idx, offset, data })
            },
            8 => {
                let piece_idx = src.get_u32() as usize;
                let offset = src.get_u32() as usize;
                let len = src.get_u32() as usize;
                Message::Cancel(block::BlockInfo { piece_idx, offset, len })
            },
            9 => Message::Port { port: src.get_u32() },
            id => {
                tracing::warn!("invalid message id: {}", id);
                return Err(PeerError::InvalidMessageId(id));
            }
        };
        
        Ok(Some(msg))
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::KeepAlive => write!(f, "keep alive"),
            Message::Choke => write!(f, "choke"),
            Message::Unchoke => write!(f, "unchoke"),
            Message::Interested => write!(f, "interested"),
            Message::NotInterested => write!(f, "not interested"),
            Message::Have { idx } => write!(f, "have piece idx: {}", idx),
            Message::Bitfield(bf) => write!(f, "bitfield with {} pieces", bf.count_ones()),
            Message::Request(block) => write!(f, "request for block {{ piece idx: {}, offset {}, length: {} }}",
                block.piece_idx, 
                block.offset, 
                block.len,
            ),
            Message::Block(block) => write!(f, "block data {{ piece idx: {}, offset: {}, length: {} }}", 
                block.piece_idx, 
                block.offset,
                block.data.len(),
            ),
            Message::Cancel(block) => write!(f, "cancel for block {{ piece idx: {}, offset: {}, length: {} }}", 
                block.piece_idx, 
                block.offset, 
                block.len
            ),
            Message::Port { port } => write!(f, "port {}", port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use bitvec::prelude::*;

    #[test]
    fn test_msg_stream() {

        let mut out_buf = BytesMut::new();
        let mut buf = BytesMut::new();
        // Keep alive
        buf.extend_from_slice(&[0, 0, 0, 0]);
        // Choke
        buf.extend_from_slice(&[0, 0, 0, 1, 0]);
        // Unchoke
        buf.extend_from_slice(&[0, 0, 0, 1, 1]);
        // Interested
        buf.extend_from_slice(&[0, 0, 0, 1, 2]);
        // Not interested
        buf.extend_from_slice(&[0, 0, 0, 1, 3]);
        // Have
        buf.extend_from_slice(&[0, 0, 0, 5, 4, 0, 0, 0, 0xb]);
        // Bitfield
        buf.extend_from_slice(&[0, 0, 0, 4, 5, 0x1, 0x2, 0x3]);
        // Request
        buf.extend_from_slice(&[0, 0, 0, 0xd, 0x6, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0, 0, 0x40, 0]);
        // Piece
        buf.extend_from_slice(&[0, 0, 0, 12, 0x7, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0x1, 0x2, 0x3]);

        let expected = [
            Message::KeepAlive,
            Message::Choke,
            Message::Unchoke,
            Message::Interested,
            Message::NotInterested,
            Message::Have { idx: 0xb },
            Message::Bitfield(BitVec::<u8, Msb0>::from_slice(&[0x1, 0x2, 0x3])),
            Message::Request(block::BlockInfo { piece_idx: 0xb, offset: 0x134000, len: 0x4000 }),
            Message::Block(block::BlockData { piece_idx: 0xb, offset: 0x134000, data: vec![0x1, 0x2, 0x3] }),
        ];
        let expected_buf = buf.clone();        
        
        for msg in expected.into_iter() {
            MessageCodec.encode(msg.clone(), &mut out_buf).unwrap();
            let decoded = MessageCodec.decode(&mut buf).unwrap().unwrap();
            assert_eq!(decoded, msg, "decoded message does not match expected");
        }
        
        assert_eq!(out_buf, expected_buf, "encoded stream does not match expected");
    }

    #[test]
    fn test_msg_decode_chunked() {
        
        let mut buf = BytesMut::new();

        // Add 1/2 of interested message
        buf.extend_from_slice(&[0, 0, 0]);
        let decoded = MessageCodec.decode(&mut buf).unwrap();
        assert_eq!(decoded, None);
        // Add other 1/2
        buf.extend_from_slice(&[1, 2]);
        let decoded = MessageCodec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded, Message::Interested);

        // Add 1/2 of piece message
        buf.extend_from_slice(&[0, 0, 0, 12, 0x7, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0x1]);
        let decoded = MessageCodec.decode(&mut buf).unwrap();
        assert_eq!(decoded, None);
        // Add other 1/2
        buf.extend_from_slice(&[0x2, 0x3]);
        let decoded = MessageCodec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded, Message::Block(block::BlockData { piece_idx: 0xb, offset: 0x134000, data: vec![0x1, 0x2, 0x3] }));
    }

    #[test]
    fn test_msg_decode_empty() {
        let mut src = BytesMut::new();
        let mut codec = MessageCodec;
        let message = codec.decode(&mut src).unwrap();
        assert_eq!(message, None);
    }

    #[test]
    fn test_msg_decode_incomplete_message() {
        let mut src = BytesMut::from(&[0u8, 1, 2][..]); // Not a complete message
        let mut codec = MessageCodec;
        let message = codec.decode(&mut src).unwrap();
        assert_eq!(message, None);
    }

    #[test]
    fn test_msg_decode_invalid_id() {
        let mut src = BytesMut::from(&[0u8, 0, 0, 1, 255][..]); // Message ID 255 is invalid
        let mut codec = MessageCodec;
        let result = codec.decode(&mut src);
        match result {
            Ok(_) => panic!("Expected an error, but got Ok(_)"),
            Err(e) => match e {
                PeerError::InvalidMessageId(id) => assert_eq!(id, 255),
                _ => panic!("Expected PeerError::InvalidMessageId, but got a different error"),
            },
        }
    }
}
