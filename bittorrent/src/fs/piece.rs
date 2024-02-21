use std::{io::{Seek, Write}, ops::Range};
use sha1::{Sha1, Digest};
use std::sync;
use crate::BLOCK_SIZE;
use super::{TorrentFile, Result};

#[derive(Debug)]
pub struct Piece {

    // Piece hash originally given in metainfo.
    pub hash: [u8; 20],

    // Length of piece in bytes.
    pub length: usize,

    // Piece data.
    pub data: Vec<u8>,

    // Indicates if a block has been received, checks for duplicates.
    pub blocks_received: Vec<bool>,

    // Number of blocks recieved.
    pub num_blocks_received: u32,

    // Range of file indices that the piece overlaps.
    pub file_overlap: Range<usize>,
}

impl Piece {

    pub fn add_block(&mut self, offset: usize, data: Vec<u8>) {
        debug_assert!(offset + data.len() <= self.length, "block offset out of range");
        let block_idx = offset / BLOCK_SIZE;
        if self.blocks_received[block_idx] {
            tracing::warn!("duplicate block at offset {}", offset);
        } else {
            self.blocks_received[block_idx] = true;
            self.num_blocks_received += 1;
            self.data[offset..offset + data.len()].copy_from_slice(&data);
        }
    }

    // Hash the piece data and compare with hash given in metainfo.
    pub fn verify_hash(&self) -> bool {
        let mut hasher = Sha1::new();
        hasher.update(&self.data);
        let hash = hasher.finalize();
        hash.as_slice() == self.hash
    }

    // Write the piece data to the files.
    pub fn write(&self, piece_offset: usize, files: &[sync::RwLock<TorrentFile>]) -> Result<()> {
        
        let mut total_offset = piece_offset;
        let mut bytes_written = 0;
        
        let files = &files[self.file_overlap.clone()];
        for file in files {
            let mut f = file.write().unwrap();
            
            let byte_range = f.info.byte_range();
            let file_offset = total_offset - byte_range.start;
            let piece_remaining = self.length - bytes_written;
            let file_remaining = byte_range.end - total_offset;
            let bytes_remaining = std::cmp::min(piece_remaining, file_remaining);
            
            // seek to the correct position in the file
            f.handle.seek(std::io::SeekFrom::Start(file_offset as u64)).unwrap();
            let n = f.handle.write(&self.data[bytes_written..bytes_written + bytes_remaining]).unwrap();
            
            total_offset += n;
            bytes_written += n;
        }
        debug_assert_eq!(bytes_written, self.length);
        Ok(())
    }
}
