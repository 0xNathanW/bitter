use std::collections::{HashSet, HashMap};
use tokio::sync::RwLock;
use crate::{block::BlockRequest, Bitfield};

pub mod piece_picker;
pub mod partial_piece;

use piece_picker::PiecePicker;
use partial_piece::PartialPiece;

#[derive(Debug)]
pub struct Picker {
    pub piece_picker:   RwLock<PiecePicker>,
    pub partial_pieces: RwLock<HashMap<usize, RwLock<PartialPiece>>>,
    num_pieces:         u32,
    piece_len:          usize,
    last_piece_len:     usize,
}

impl Picker {

    pub fn new(num_pieces: u32, piece_len: usize, last_piece_len: usize) -> Self {
        Self {
            piece_picker: RwLock::new(PiecePicker::new(num_pieces as usize)),
            partial_pieces: RwLock::new(HashMap::new()),
            num_pieces,
            piece_len,
            last_piece_len,
        }
    }

    pub async fn pick_blocks(
        &self,
        current_requests: &HashSet<BlockRequest>,
        target_queue_len: usize,
        bf: &Bitfield,
    ) -> Vec<BlockRequest> {

        let mut requests = vec![];
        let mut remaining = target_queue_len.saturating_sub(current_requests.len());
        if remaining == 0 {
            return vec![];
        }

        // Attempt to pick blocks from partially downloaded pieces.
        for partial_piece in self.partial_pieces.write().await.values_mut() {
            
            // Target queue length reached.
            if remaining == 0 {
                break;
            }
            
            // Skip pieces that peer does not have.
            if !bf[partial_piece.read().await.idx as usize] {
                continue;
            }

            remaining -= partial_piece
                .write()
                .await
                .pick_next_blocks(remaining, &mut requests, &current_requests, false);
        }
        
        // Pick blocks from new pieces.
        while remaining != 0 {
            
            if let Some(idx) = self.piece_picker.write().await.pick_new_piece(bf) {
                tracing::trace!("picked piece {}", idx);
                // Begin a new partial piece.
                let mut partial_piece = PartialPiece::new(idx, if idx as u32 == self.num_pieces - 1 { self.last_piece_len } else { self.piece_len });
                remaining -= partial_piece.pick_next_blocks(remaining, &mut requests, current_requests, false);
                self.partial_pieces.write().await.insert(idx, partial_piece.into());
            
            } else {
                // End game if all pieces have been picked.
                for partial_piece in self.partial_pieces.write().await.values_mut() {
                    
                    if remaining == 0 {
                        return requests;
                    }
                    if !bf[partial_piece.read().await.idx as usize] {
                        continue;
                    }
                    
                    remaining -= partial_piece
                        .write()
                        .await
                        .pick_next_blocks(remaining, &mut requests, &current_requests, true);
                }
                return requests;
            }
        }
        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BLOCK_SIZE;
    use bitvec::prelude::*;

    #[tokio::test]
    async fn test_pick_blocks() {
        let picker = Picker::new(1028, 32_768, 32_768);
        let bf = BitVec::repeat(true, 1028);
        picker.piece_picker.write().await.bitfield_update(&bf);
        let requests_1 = picker.pick_blocks(&HashSet::new(), 4, &bf).await;
        assert_eq!(requests_1.len(), 4);
        let requests_2 = picker.pick_blocks(&HashSet::new(), 4, &bf).await;
        assert_eq!(requests_2.len(), 4);
    }

    #[tokio::test]
    async fn test_pick_blocks_end_game() {
        
        let picker = Picker::new(2, 32_768, 32_768);
        let bf = BitVec::repeat(true, 2);
        picker.piece_picker.write().await.bitfield_update(&bf);
        
        // Pick all the blocks.
        let requests_1 = picker.pick_blocks(&HashSet::new(), 4, &bf).await;
        assert_eq!(requests_1.len(), 4);
        
        // Try endgame.
        let requests_2 = picker.pick_blocks(&HashSet::new(), 4, &bf).await;
        assert_eq!(requests_2.len(), 4);
        
        // Endgame with blocks already in queue.
        let mut previous_requests = HashSet::new();
        previous_requests.insert(BlockRequest { piece_idx: 0, offset: 0, len: BLOCK_SIZE });
        previous_requests.insert(BlockRequest { piece_idx: 1, offset: 0, len: BLOCK_SIZE });
        let requests_3 = picker.pick_blocks(&previous_requests, 4, &bf).await;
        assert_eq!(requests_3.len(), 2);
    }
}
