use super::{Result, Error};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Have {
        idx: u32,
    },

    // The bitfield message is a short form method of communicating to a peer what pieces 
    // a client has usually sent after the handshake has been completed.
    Bitfield {
        bitfield: Vec<u8>,
    },

    // When a client wants to request data, they reference the index of the piece, the index 
    // of the start of the block within the piece, and the length of the block (usually 16KB).
    Request {
        idx:    u32,
        begin:  u32,
        length: u32,
    },

    // Clients send blocks in the piece message, referencing piece index and block offset.
    Piece {
        idx:    u32,
        begin:  u32,
        block:  Vec<u8>,
    },

    // The cancel message is sent to cancel a request for a block.
    Cancel {
        idx:    u32,
        begin:  u32,
        length: u32,
    },

    // The port message is sent to inform the peer of the port number that the client is listening on.
    Port {
        port: u32,
    },
}

impl Message {

    // Encode message to bytes, consume self.
    pub fn encode(self) -> Vec<u8> {
        match self {
            Message::KeepAlive      => vec![0; 4],
            Message::Choke          => vec![0, 0, 0, 1, 0],
            Message::Unchoke        => vec![0, 0, 0, 1, 1],
            Message::Interested     => vec![0, 0, 0, 1, 2],
            Message::NotInterested  => vec![0, 0, 0, 1, 3],

            // have: <len=0005><id=4><piece index>
            Message::Have { idx } => {
                let mut buf = vec![0, 0, 0, 5, 4];
                buf.extend(&idx.to_be_bytes());
                buf
            },

            // bitfield: <len=0001+X><id=5><bitfield>
            Message::Bitfield { bitfield } => {
                let mut buf = vec![0, 0, 0, 1 + bitfield.len() as u8, 5];
                buf.extend(bitfield.consume());
                buf
            },

            // request: <len=0013><id=6><index><begin><length>            
            Message::Request { idx, begin, length } => {
                let mut buf = vec![0, 0, 0, 13, 6];
                buf.extend(&idx.to_be_bytes());
                buf.extend(&begin.to_be_bytes());
                buf.extend(&length.to_be_bytes());
                buf
            },

            // piece: <len=0009+X><id=7><index><begin><block>
            Message::Piece { idx, begin, mut block } => {
                let mut buf = vec![0, 0, 0, 9 + block.len() as u8, 7];
                buf.extend(&idx.to_be_bytes());
                buf.extend(&begin.to_be_bytes());
                buf.append(&mut block);
                buf
            },

            // cancel: <len=0013><id=8><index><begin><length>
            Message::Cancel { idx, begin, length } => {
                let mut buf = vec![0, 0, 0, 13, 8];
                buf.extend(&idx.to_be_bytes());
                buf.extend(&begin.to_be_bytes());
                buf.extend(&length.to_be_bytes());
                buf
            },

            // port: <len=0003><id=9><listen-port>
            Message::Port { port } => {
                let mut buf = vec![0, 0, 0, 3, 9];
                buf.extend(&port.to_be_bytes());
                buf
            },
        }
    }

    // Decode message from bytes, return self.
    pub fn decode(buf: &[u8]) -> Result<Self> {
        match buf[0] {
            0 => Ok(Message::Choke),
            1 => Ok(Message::Unchoke),
            2 => Ok(Message::Interested),
            3 => Ok(Message::NotInterested),
            4 => Ok(Message::Have { idx: u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) }),
            5 => Ok(Message::Bitfield { bitfield: buf[1..].to_vec().into() }),
            6 => Ok(Message::Request {
                idx:    u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]),
                begin:  u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]),
                length: u32::from_be_bytes([buf[9], buf[10], buf[11], buf[12]]),
            }),
            7 => Ok(Message::Piece {
                idx:    u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]),
                begin:  u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]),
                block:  buf[9..].to_vec(),
            }),
            8 => Ok(Message::Cancel {
                idx:    u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]),
                begin:  u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]),
                length: u32::from_be_bytes([buf[9], buf[10], buf[11], buf[12]]),
            }),
            9 => Ok(Message::Port { port: u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) }),
            _ => Err(Error::InvalidMessageID(buf[0])),
        }
    }

    pub fn fmt_short(&self) -> String {
        match self {
            Message::KeepAlive            => "KeepAlive".to_string(),
            Message::Choke                => "Choke".to_string(),
            Message::Unchoke              => "Unchoke".to_string(),
            Message::Interested           => "Interested".to_string(),
            Message::NotInterested        => "NotInterested".to_string(),
            Message::Have { idx }   => format!("Have({})", idx),
            Message::Bitfield { .. }      => "Bitfield".to_string(),
            Message::Request { .. }       => "Request".to_string(),
            Message::Piece { .. }         => "Piece".to_string(),
            Message::Cancel { .. }        => "Cancel".to_string(),
            Message::Port { port }  => format!("Port({})", port),
        }
    }

    pub fn fmt_long(&self) -> String {
        match self {
            Message::KeepAlive                                            => "KeepAlive".to_string(),
            Message::Choke                                                => "Choke".to_string(),
            Message::Unchoke                                              => "Unchoke".to_string(),
            Message::Interested                                           => "Interested".to_string(),
            Message::NotInterested                                        => "NotInterested".to_string(),
            Message::Have { idx }                                   => format!("Have({})", idx),
            Message::Bitfield { bitfield: _ }                             => format!("Bitfield"),
            Message::Request { idx, begin, length }     => format!("Request(idx: {}, begin: {}, length: {})", idx, begin, length),
            Message::Piece { idx, begin, block }    => format!("Piece(idx: {}, begin: {}, length: {})", idx, begin, block.len()),
            Message::Cancel { idx, begin, length }      => format!("Cancel(idx: {}, begin: {}, length: {})", idx, begin, length),
            Message::Port { port }                                  => format!("Port({})", port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode() {
        assert_eq!(Message::KeepAlive.encode(), vec![0; 4]);
        assert_eq!(Message::Choke.encode(), vec![0, 0, 0, 1, 0]);
        assert_eq!(Message::Unchoke.encode(), vec![0, 0, 0, 1, 1]);
        assert_eq!(Message::Interested.encode(), vec![0, 0, 0, 1, 2]);
        assert_eq!(Message::NotInterested.encode(), vec![0, 0, 0, 1, 3]);
        assert_eq!(Message::Have { idx: 0xb }.encode(), vec![0, 0, 0, 5, 4, 0, 0, 0, 0xb]);
        assert_eq!(
            Message::Bitfield { bitfield: vec![0x1, 0x2, 0x3] }.encode(),
            vec![0, 0, 0, 4, 5, 0x1, 0x2, 0x3]
        );
        assert_eq!(
            Message::Request { idx: 0xb, begin: 0x134000, length: 0x4000 }.encode(),
            vec![0, 0, 0, 0xd, 0x6, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0, 0, 0x40, 0]
        );
        assert_eq!(
            Message::Piece { idx: 0xb, begin: 0x134000, block: vec![0x1, 0x2, 0x3] }.encode(),
            vec![0, 0, 0, 12, 0x7, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0x1, 0x2, 0x3]
        );
    }

    #[test]
    fn test_decode() {
        assert_eq!(Message::decode(&[0]).unwrap(), Message::Choke);
        assert_eq!(Message::decode(&[1]).unwrap(), Message::Unchoke);
        assert_eq!(Message::decode(&[2]).unwrap(), Message::Interested);
        assert_eq!(Message::decode(&[3]).unwrap(), Message::NotInterested);
        assert_eq!(Message::decode(&[4, 0, 0, 0, 0xb]).unwrap(), Message::Have { idx: 0xb });
        assert_eq!(Message::decode(&[0x6, 0, 0, 0, 0xb, 0, 0x13, 0x40, 0, 0, 0, 0x40, 0]).unwrap(),
            Message::Request { idx: 0xb, begin: 0x134000, length: 0x4000 }
        );
    }
}