use std::fs;
use std::sync::Arc;
use std::thread::spawn;
use tokio::sync::{mpsc, Mutex};
use tokio::task::spawn_blocking;

use super::{
    {Result, Error},
    peer::Peer,
    message::Message,
    request::{Request, Action},
};

const BLOCK_LEN: u32 = 16384; // 16Kb

impl Peer {

    pub async fn trade_pieces(
        &mut self,
        workload: PieceWorkQueue,
        fs_out: mpsc::Sender<PieceData>,
        requests: mpsc::Sender<Request>,
    ) -> Result<()> {

        // Make sure the peer is not choking us.
        if self.choked {
            // Attempt unchoke.
            self.send(Message::Interested).await?;
            let msg = self.recv().await?;
            match msg {
                Message::Unchoke => self.choked = false,
                _ => return Err(Error::Choke),
            }
        }

        while let Some(piece) = workload.next().await {
            if !self.has_piece(piece.idx) {
                workload.push(piece).await;
                continue;
            }
            if self.download_piece(piece.clone(), &requests, &fs_out).await.is_err() {
                workload.push(piece).await;
                continue;
            }
            if let Some(chan) = &self.display_chan {
                let _ = chan.send(format!("Downloaded piece {}.", piece.idx)).await;
            }
        }
        // Implement a barrier here.
        

        Ok(())
    }

    // Pieces are too long to request in one go.
	// We will request a piece in chunks of 16384 bytes (16Kb) called blocks.
	// The last block will likely be smaller.
    pub async fn download_piece(
        &mut self, 
        piece: Piece,
        requests: &mpsc::Sender<Request>,
        fs_out: &mpsc::Sender<PieceData>,
    ) -> Result<()> {

        let piece_len: u32 = piece.end - piece.begin;
        let mut piece_data = vec![0_u8; piece_len as usize];
        let mut requested = 0_u32;
        let mut downloaded = 0_u32;

        // Request all blocks in piece.
        while requested < piece_len {
            // If last block is smaller, set block size to remaining bytes.
            let block_len = if requested + BLOCK_LEN > piece_len {
                piece_len - requested
            } else {
                BLOCK_LEN
            };
            self.send(
                Message::Request { 
                    idx: piece.idx, 
                    begin: requested, 
                    length: block_len }
            ).await?;
            requested += block_len;
        }

        // Read responses containing block.
        while downloaded < piece_len {

            if self.peer_choking {
                self.attempt_unchoke().await?;
            }

            let msg = self.recv().await?;
            match msg {

                Message::Piece { idx, begin, block } => {
                    if idx != piece.idx { 
                        return Err(Error::PieceError(piece::Error::IndexMismatch(idx, piece.idx))) 
                    }
                    if begin >= piece_len {
                        return Err(Error::PieceError(piece::Error::BlockInvalidBounds(
                            format!("begin {}, exceeds the piece length {}", begin, piece_len)
                        )));
                    }
                    let end = begin + block.len() as u32;
                    if end > piece_len {
                        return Err(Error::PieceError(piece::Error::BlockInvalidBounds(
                            format!("end {}, exceeds piece length {}", end, piece_len)
                        )));
                    }
                    downloaded += block.len() as u32;
                    piece_data.splice(begin as usize .. end as usize, block.into_iter());
                }

                Message::Request { idx, begin, length } => {
                    let _ = requests.send(Request::new(idx, begin, length, Action::Request)).await;
                },

                Message::Cancel { idx, begin, length } => {
                    let _ = requests.send(Request::new(idx, begin, length, Action::Cancel)).await;
                },

                _ => self.handle_msg(msg),
            }
        }

        let cloned_piece_data = piece_data.clone();
        spawn_blocking(move || { piece.verify_hash(&cloned_piece_data) }).await.unwrap()?;

        fs_out.send(PieceData { idx: piece.idx, data: piece_data }).await?;
        Ok(())
    }
}

