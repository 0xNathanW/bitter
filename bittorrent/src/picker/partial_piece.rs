use std::collections::HashSet;
use crate::{block::{BlockInfo, block_len, num_blocks}, BLOCK_SIZE};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum BlockState {
    #[default]
    Free,
    Requested,
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
    
    pub fn free_block(&mut self, block: &BlockInfo) {
        assert!(block.piece_idx == self.idx);
        self.blocks_states[block.idx_in_piece()] = BlockState::Free
    }

    pub fn free_all_blocks(&mut self) {
        self.blocks_states.iter_mut().for_each(|b| *b = BlockState::Free)
    }
    
    // Returns the state the block was previously in, to check for duplicates.
    pub fn received_block(&mut self, block: &BlockInfo) -> BlockState {
        assert!(block.piece_idx == self.idx);
        let block_state = &mut self.blocks_states[block.idx_in_piece()];
        assert!(*block_state != BlockState::Free);
        let prev_state = *block_state;
        *block_state = BlockState::Received;
        prev_state
    }

    // Pick open blocks sequentially within a partially downloaded piece.
    pub fn pick_next_blocks(
        &mut self,
        num: usize,
        buf: &mut Vec<BlockInfo>,
        prev: &HashSet<BlockInfo>,
        end_game: bool,
    ) -> usize {
        let mut num_picked = 0;
        for (i, block) in self.blocks_states.iter_mut().enumerate() {
            if num_picked == num {
                break;
            }
            
            if *block == BlockState::Free {
                assert!(!end_game);
                buf.push(BlockInfo {
                    piece_idx: self.idx,
                    offset: i * BLOCK_SIZE as usize,
                    len: block_len(self.len, i)
                });
                *block = BlockState::Requested;
                num_picked += 1;

            } else if end_game && *block == BlockState::Requested {
                
                let block_info = BlockInfo {
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
