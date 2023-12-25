use std::collections::HashSet;
use crate::{block::{BlockInfo, block_size, num_blocks}, BLOCK_SIZE};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum BlockState {
    #[default]
    Open,
    Requested,
    Received,
}

#[derive(Debug)]
pub struct PartialPiece {
    // Piece index.
    idx: usize,
    // Length of piece.
    len: u32,
    // State of all blocks within this piece.
    blocks_states: Vec<BlockState>
}

impl PartialPiece {

    pub fn new(idx: usize, len: u32) -> Self {
        Self {
            idx,
            len,
            blocks_states: vec![BlockState::default(); num_blocks(len) as usize],
        }
    }
    
    pub fn free_block(&mut self, block: &BlockInfo) {
        self.blocks_states[block.idx_in_piece()] = BlockState::Open
    }

    pub fn free_all_blocks(&mut self) {
        self.blocks_states.iter_mut().for_each(|b| *b = BlockState::Open)
    }

    // Pick open blocks within a partially downloaded piece.
    pub fn pick_blocks_in_partial_piece(
        &mut self,
        num: usize,
        buf: &mut Vec<BlockInfo>,
        prev: &HashSet<BlockInfo>,
    ) -> Option<usize> {
        let mut num_picked = 0;
        for (i, block) in self.blocks_states.iter_mut().enumerate() {
            if num_picked == num {
                return None;
            }
            if *block == BlockState::Open {
                buf.push(BlockInfo {
                    piece_idx: self.idx,
                    offset: i * BLOCK_SIZE as usize,
                    len: block_size(self.len, i)
                });
                *block = BlockState::Requested;
                num_picked += 1;
            }
        }
        Some(num - num_picked)
    }

    pub fn received_block(&mut self, block: &BlockInfo) {
        self.blocks_states[block.idx_in_piece()] = BlockState::Received;
    }
}
