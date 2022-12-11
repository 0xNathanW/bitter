use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::piece::{PieceWorkQueue, Piece, self};
use super::{Result, Error};
use super::peer::Peer;
use super::message::Message;
use super::request::Request;

const BLOCK_LEN: u32 = 16384; // 16Kb

impl Peer {

    pub async fn trade_pieces(
        &mut self,       
        workload: PieceWorkQueue,
        fs_out: mpsc::Sender<String>,
        requests: mpsc::Sender<Request>,
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
    pub async fn download_piece(&mut self, piece: Piece, requests: mpsc::Sender<Request>) -> Result<()> {

        let piece_len: u32 = piece.end - piece.begin;
        let mut piece_data = Vec::<u8>::with_capacity(piece_len as usize);
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
            let msg = self.recv().await?;
            match msg {
                Message::KeepAlive => {},

                Message::Choke => {
                    self.choked = true;
                    return Err(Error::Choke);
                }
                Message::Unchoke => self.choked = false,

                Message::Bitfield { bitfield } => self.set_bitfield(bitfield),
                Message::Have { idx } => self.set_piece(idx),

                Message::Interested => self.interested = true,
                Message::NotInterested => self.interested = false,

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
                    requests.send(Request { idx, begin, length }).await;
                },

                Message::Cancel { idx, begin, length } => {

                },

                Message::Port { port } => self.new_port(port as u16),
            }
        }

        // Send piece data to hasher.
        Ok(())
    }
}

