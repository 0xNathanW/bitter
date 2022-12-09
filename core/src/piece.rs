use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;

use super::torrent::Torrent;

// Piece of the torrent data.
pub struct Piece {
    pub idx:    usize,
    pub hash:   [u8; 20],
    pub begin:  usize,
    pub end:    usize,
}

// Piece data received from peers.
pub struct PieceData {
    pub idx:  usize,
    pub data: Vec<u8>,
}

pub struct PieceWorkQueue(Arc<Mutex<VecDeque<Piece>>>);

impl PieceWorkQueue {

    pub fn new(torrent: &Torrent) -> Self {
        let mut pieces = VecDeque::new();
        let piece_len = torrent.piece_length();
        
        for (idx, hash) in torrent.pieces_iter().enumerate() {
            let begin = idx * piece_len as usize;
            let mut end = begin + piece_len as usize;   
            if end > torrent.size() as usize {
                end = torrent.size() as usize;
            }
            // This is safe because we know pieces is a multiple of 20.
            let hash = hash.try_into().unwrap();
            pieces.push_back(Piece { idx, hash, begin, end });
        }

        Self(Arc::new(Mutex::new(pieces)))
    }

    pub async fn next(&self) -> Option<Piece> {
        let mut pieces = self.0.lock().await;
        pieces.pop_front()
    }

    pub async fn push(&self, piece: Piece) {
        let mut pieces = self.0.lock().await;
        pieces.push_front(piece);
    }
}
