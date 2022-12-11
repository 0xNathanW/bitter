use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;
use thiserror::Error;

use super::torrent::Torrent;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Index mismatch: expected {0}, got {1}")]
    IndexMismatch(u32, u32),

    #[error("Recieved block has invalid bounds: {0}")]
    BlockInvalidBounds(String)
}


// Piece of the torrent data.
#[derive(Debug)]
pub struct Piece {
    pub idx:    u32,
    pub hash:   [u8; 20],
    pub begin:  u32,
    pub end:    u32,
}

// Piece data received from peers.
pub struct PieceData {
    pub idx:  u32,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct PieceWorkQueue(pub Arc<Mutex<VecDeque<Piece>>>);

impl PieceWorkQueue {

    pub fn new(torrent: &Torrent) -> Self {
        let mut pieces = VecDeque::new();
        let piece_len = torrent.piece_length() as u32;
        
        for (idx, hash) in torrent.pieces_iter().enumerate() {
            let idx = idx as u32;
            let begin = idx * piece_len;
            let mut end = begin + piece_len;   
            if end > torrent.size() as u32 {
                end = torrent.size() as u32;
            }
            // This is safe because we know pieces is a multiple of 20.
            let hash = hash.try_into().unwrap();
            pieces.push_back(Piece { idx: idx as u32, hash, begin, end });
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

impl Clone for PieceWorkQueue {
    fn clone(&self) -> Self {
        PieceWorkQueue(self.0.clone())    
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::torrent::Torrent;
    use std::path::Path;
    use rand::Rng;

    #[tokio::test]
    async fn piece_work_queue() {

        let torrent_path = Path::new("../test_torrents/test_single_file.torrent");
        let torrent = Torrent::new(torrent_path).unwrap();
        println!("{}", torrent.piece_length());
        let work_queue = PieceWorkQueue::new(&torrent); 

        for w in 0..5 {
            let queue = work_queue.clone();

            tokio::spawn(async move {
                while let Some(piece) = queue.next().await {
                    println!("{}: popped {:?}", w, piece);
                    if rand::random::<u8>() > 200 {
                        println!("{}: pushed {:?}", w, piece);
                        queue.push(piece).await;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            });
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1000)).await
    }

}
