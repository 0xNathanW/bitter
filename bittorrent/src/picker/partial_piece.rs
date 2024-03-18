use std::collections::HashSet;
use crate::{block::*, BLOCK_SIZE};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum BlockState {
    
    // Block has not been requested.
    #[default]
    Free,
    
    // Block has been requested by at least 1 peer.
    Requested,
    
    // Block has been received.
    Received,

}

#[derive(Debug)]
pub struct PartialPiece {

    // Piece index.
    pub idx: usize,
    
    // Length of piece.
    pub len: usize,
    
    // State of all blocks within this piece.
    pub blocks_states: Vec<BlockState>

}

impl PartialPiece {

    pub fn new(idx: usize, len: usize) -> Self {
        Self {
            idx,
            len,
            blocks_states: vec![BlockState::default(); num_blocks(len) as usize],
        }
    }
    
    pub fn free_block(&mut self, block: &BlockRequest) {
        assert!(block.piece_idx == self.idx);
        self.blocks_states[block.idx_in_piece()] = BlockState::Free
    }

    pub fn free_all_blocks(&mut self) {
        self.blocks_states.iter_mut().for_each(|b| *b = BlockState::Free)
    }
    
    // Returns whether the block is a duplicate (already recieved).
    pub fn received_block(&mut self, block: &BlockRequest) -> bool {
        let block_state = &mut self.blocks_states[block.idx_in_piece()];
        // If we received a block, it must have been requested.
        match *block_state {
            BlockState::Free => unreachable!("Can't receive a block that wasn't requested"),
            BlockState::Requested => {
                *block_state = BlockState::Received;
                false
            },
            BlockState::Received => true,
        }
    }

    // Pick open blocks sequentially within a partially downloaded piece.
    pub fn pick_next_blocks(
        &mut self,
        num: usize,
        buf: &mut Vec<BlockRequest>,
        prev: &HashSet<BlockRequest>,
        end_game: bool,
    ) -> usize {
        let mut num_picked = 0;
        for (i, block) in self.blocks_states.iter_mut().enumerate() {
            if num_picked == num {
                break;
            }
            
            if *block == BlockState::Free {
                assert!(!end_game);
                buf.push(BlockRequest {
                    piece_idx: self.idx,
                    offset: i * BLOCK_SIZE as usize,
                    len: block_len(self.len, i)
                });
                *block = BlockState::Requested;
                num_picked += 1;

            } else if end_game && *block == BlockState::Requested {
                
                let block_info = BlockRequest {
                    piece_idx: self.idx,
                    offset: i * BLOCK_SIZE as usize,
                    len: block_len(self.len, i),
                };
                
                if !prev.contains(&block_info) {
                    buf.push(block_info);
                    num_picked += 1;
                }
            }
        }
        num_picked
    }
}
