use std::collections::{HashSet, HashMap};
use crate::block::BlockInfo;
use super::partial_piece::PartialPiece;

#[derive(Debug)]
pub struct BlockPicker {
    pub partial_pieces: HashMap<usize, PartialPiece>
}

impl BlockPicker {
    pub fn new() -> Self {
        Self { partial_pieces: HashMap::new() }
    }

    pub fn pick_blocks(
        &mut self,
        num: usize,
        requests: &mut Vec<BlockInfo>,
        current_requests: &HashSet<BlockInfo>,
    ) -> Option<usize> {
        let mut num_picked = 0;
        for download in self.partial_pieces.values_mut() {
            let remaining = download.pick_blocks_in_partial_piece(
                num - num_picked,
                requests,
                &current_requests,
            )?;
        }
        Some(num - num_picked)
    }
}
