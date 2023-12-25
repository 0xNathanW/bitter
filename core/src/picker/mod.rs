
pub mod block_picker;
pub mod piece_picker;
pub mod partial_piece;

use std::collections::HashSet;
use tokio::sync::RwLock;
use crate::block::BlockInfo;

use piece_picker::PiecePicker;
use block_picker::BlockPicker;
use partial_piece::PartialPiece;

#[derive(Debug)]
pub struct Picker {
    pub piece_picker: RwLock<PiecePicker>,
    pub block_picker: RwLock<BlockPicker>,
    piece_len:    u32,
}

impl Picker {

    pub fn new(num_pieces: usize, piece_len: u32) -> Self {
        Self {
            piece_picker: RwLock::new(PiecePicker::new(num_pieces)),
            block_picker: RwLock::new(BlockPicker::new()),
            piece_len,
        }
    }

    pub async fn pick_blocks(
        &self,
        current_requests: &HashSet<BlockInfo>,
        target_queue_len: usize,
    ) -> Vec<BlockInfo> {
        let mut requests = vec![];
        // Attempt to pick blocks from partially downloaded pieces.
        let mut remaining = self.block_picker
            .write()
            .await
            .pick_blocks(target_queue_len - current_requests.len(), &mut requests, current_requests);
        // Pick blocks from new pieces.
        while let Some(num) = remaining {

            if let Some(idx) = self.piece_picker.write().await.pick_piece() {
                tracing::info!("picked piece idx: {}", idx);

                let mut partial_piece = PartialPiece::new(idx, self.piece_len);
                let new_remaining = partial_piece.pick_blocks_in_partial_piece(num, &mut requests, current_requests);
                remaining = new_remaining;
                self.block_picker.write().await.partial_pieces.insert(idx, partial_piece);
            }
        }
        requests
    }
}
