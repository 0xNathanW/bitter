use std::{io::{Read, Seek, Write}, sync::Arc};
use sha1::{Sha1, Digest};
use crate::{block::Block, BLOCK_SIZE, ID};
use super::{torrent::TorrentFile, Result};

#[derive(Debug)]
pub struct PieceBuf {

    // Piece hash originally given in metainfo.
    pub hash: ID,

    // Length of piece in bytes.
    pub len: usize,

    // Piece data.
    pub data: Vec<u8>,

    // Indicates if a block has been received, checks for duplicates.
    pub blocks_received: Vec<bool>,

    // Number of blocks recieved.
    pub num_blocks_received: u32,

    // Range of file indices that the piece overlaps.
    pub file_range: std::ops::Range<usize>,

}

impl PieceBuf {

    pub fn add_block(&mut self, block: &Block) {
        let block_idx = block.offset / BLOCK_SIZE;
        if self.blocks_received[block_idx] {
            tracing::warn!("duplicate block in piece {} at offset {}", block.piece_idx, block.offset);
        } else {
            self.blocks_received[block_idx] = true;
            self.num_blocks_received += 1;
            self.data[block.offset..block.offset + block.data.len()].copy_from_slice(block.data.as_ref());
        }
    }

    pub fn is_complete(&self) -> bool {
        self.num_blocks_received == self.blocks_received.len() as u32
    }

    // Hash the piece data and compare with hash given in metainfo (computationally expensive).
    pub fn verify_hash(&self) -> bool {
        let mut hasher = Sha1::new();
        hasher.update(&self.data);
        let hash = hasher.finalize();
        hash.as_slice() == self.hash
    }

    pub fn write(&self, piece_offset: usize, files: &[TorrentFile]) -> Result<()> {
        
        let mut total_offset = piece_offset;
        let mut bytes_written = 0;
        
        for file in files {
            let mut f = file.file_lock.write()?;
            
            let byte_range = file.byte_range();
            let file_offset = total_offset - byte_range.start;
            let piece_remaining = self.len - bytes_written;
            let file_remaining = byte_range.end - total_offset;
            let bytes_remaining = std::cmp::min(piece_remaining, file_remaining);
            
            // seek to the correct position in the file
            // TODO: do we only have to seek on the first file?
            f.seek(std::io::SeekFrom::Start(file_offset as u64))?;
            let n = f.write(&self.data[bytes_written..bytes_written + bytes_remaining])?;
            
            total_offset += n;
            bytes_written += n;
        }
        
        if bytes_written != self.len {
            return Err(super::DiskError::IoSizeError {
                expected: self.len,
                actual: bytes_written,
            });
        }

        Ok(())
    }
}

// Reads n contiguous bytes from files.
pub fn read_piece(
    offset: usize,
    len: usize,
    files: &[TorrentFile],
) -> Result<Vec<Arc<Vec<u8>>>> {
    
    let mut bytes_read: usize = 0;
    let mut total_offset = offset;
    let mut buf = vec![0; len];

    for file in files.iter() {
        let mut f = file.file_lock.write()?;
        let byte_range = file.byte_range();

        let file_offset = total_offset.checked_sub(byte_range.start).ok_or(super::DiskError::IoSizeError {
            expected: byte_range.start,
            actual: total_offset,
        })?;
        
        let piece_remaining = len - bytes_read;
        let file_remaining = byte_range.end - total_offset;
        let bytes_remaining = std::cmp::min(piece_remaining, file_remaining);

        // TODO: can this be skipped after first file (idx = 0)?.
        f.seek(std::io::SeekFrom::Start(file_offset as u64))?;
        let n = f.read(&mut buf[bytes_read..bytes_read + bytes_remaining])?;

        bytes_read += n;
        total_offset += n;
    }
    
    if bytes_read != len {
        return Err(super::DiskError::IoSizeError {
            expected: len,
            actual: bytes_read,
        });
    }
    
    Ok(buf.chunks(BLOCK_SIZE as usize)
        .map(|chunk| Arc::new(chunk.to_vec()))
        .collect())
}
