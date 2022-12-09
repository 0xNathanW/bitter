use std::vec;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {

    #[error("Incorrect length, expected 68, got {0}")]
    IncorrectLength(usize),

    #[error("Incorrect protocol, expected \"BitTorrent protocol\", got {0}")]
    IncorrectProtocol(String),

    #[error("Incorrect info hash, expected {expected:?}, got {got:?}")]
    IncorrectInfoHash {
        expected: [u8; 20],
        got:      [u8; 20],
    },
}

pub fn handshake(info_hash: [u8; 20], id: [u8; 20]) -> Vec<u8> {
    let mut buf = vec![0; 68];
    buf[0] = 19;
    buf[1..20].copy_from_slice(b"BitTorrent protocol");
    buf[28..48].copy_from_slice(&info_hash);
    buf[48..68].copy_from_slice(&id);
    buf
}

pub fn verify_handshake(msg: Vec<u8>, info_hash: [u8; 20]) -> Result<(), Error> {
    if msg.len() != 68 {
        return Err(Error::IncorrectLength(msg.len()));
    }
    if msg[1..20] != b"BitTorrent protocol".to_vec() || msg[0] != 19 {
        return Err(Error::IncorrectProtocol(String::from_utf8_lossy(msg[0..20].as_ref()).to_string()));
    }
    if msg[28..48] != info_hash {
        return Err(Error::IncorrectInfoHash {
            expected:   info_hash,
            got:        msg[28..48].try_into().unwrap(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_handshake_out() {
        let expected = hex::decode("13426974546f7272656e742070726f746f636f6c0000000000000000bd00ed1cf18e575a5cb829d4349bceed34d768332670bdf28d63649ccc9a0face3627c6b1f55eb04");
        let info_hash = hex::decode("bd00ed1cf18e575a5cb829d4349bceed34d76833").unwrap().try_into().unwrap();
        let id = hex::decode("2670bdf28d63649ccc9a0face3627c6b1f55eb04").unwrap().try_into().unwrap();
        let msg = handshake(info_hash, id);
        assert_eq!(msg, expected.unwrap());
    }

    #[test]
    fn test_handshake() {
        let info_hash = [0; 20];
        let id = [0; 20];
        let msg = handshake(info_hash, id);
        verify_handshake(msg, info_hash).unwrap();
    }
}