use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::piece::PieceWorkQueue;
use super::{Result, Error};
use super::peer::Peer;

const BLOCK_LEN: usize = 16384; // 16Kb

impl Peer {

    pub async fn run(
        &mut self,       
        workload: PieceWorkQueue,
        fs_out: mpsc::Sender<String>,
    ) -> Result<()> {

        while let Some(piece) = workload.next().await {
            if !self.has_piece(piece.idx) {
                workload.push(piece).await;
                continue;
            }


        }

        Ok(())
    }

    // Pieces are too long to request in one go.
	// We will request a piece in chunks of 16384 bytes (16Kb) called blocks.
	// The last block will likely be smaller.
    pub async fn download_piece(&mut self, piece: Piece) -> Result<()> {
        let piece_len = piece.end - piece.begin;
        let mut piece_data = Vec::<u8>::with_capacity(piece_len);
        let mut requested = 0;

        while requested < piece_len {
            let block_len = if piece_len > 16384 { 16384 } else { piece_len };
            let block = self.request_block(piece.idx, piece_begin, block_len).await?;
            piece_data.extend_from_slice(&block);
            piece_len -= block_len;
            piece_begin += block_len;
        }


        Ok(())
    }
}

