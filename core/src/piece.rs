use super::Torrent;
use crossbeam::deque::{Injector, Steal, Stealer, Worker};

// Piece of the torrent data.
pub struct Piece {
    pub idx:    usize,
    pub hash:   [u8; 20],
    pub length: usize,
    pub begin:  usize,
    pub end:    usize,
}

// Piece data received from peers.
pub struct PieceData {
    pub idx:  usize,
    pub data: Vec<u8>,
}

impl Torrent {

    pub fn new_workload(&self) {
        
    }

}
