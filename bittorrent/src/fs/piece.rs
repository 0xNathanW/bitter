use core::num;
use std::{ops::Range, io::IoSlice, collections::BTreeMap};
use sha1::{Sha1, Digest};
use std::sync;
use super::file::TorrentFile;

#[derive(Debug)]
pub struct Piece {

    pub hash: [u8; 20],

    pub length: usize,

    pub blocks: BTreeMap<usize, Vec<u8>>,

    pub file_overlap: Range<usize>,
}

impl Piece {
    // TODO: return error?.
    pub fn add_block(&mut self, offset: usize, data: Vec<u8>) {
        use std::collections::btree_map::Entry;
        let entry = self.blocks.entry(offset);
        if matches!(entry, Entry::Occupied(_)) {
            tracing::warn!("duplicate block at offset {}", offset);
        } else {
            entry.or_insert(data);
        }
    }

    // Hash the piece data and compare with hash given in metainfo.
    pub fn verify_hash(&self) -> bool {
        let mut hasher = Sha1::new();
        for block in self.blocks.values() {
            hasher.update(&block);
        }
        let hash = hasher.finalize();
        hash.as_slice() == self.hash
    }
    
    pub fn write(&self, piece_offset: usize, files: &[sync::RwLock<TorrentFile>]) {
        let files = &files[self.file_overlap.clone()];
        let mut offset = piece_offset;
        let mut overflow_block: Option<Vec<u8>> = None;
        let mut buf = vec![];
    
        for file in files {
            let mut f = file.write().unwrap();
            let byte_range = f.info.byte_range();
            let mut num_bytes = 0;
    
            if let Some(block) = overflow_block.take() {
                if offset + block.len() > byte_range.end {
                    let overflow_size = offset + block.len() - byte_range.end;
                    let in_current_file = block.len() - overflow_size;
                    buf.push(block[..in_current_file].to_vec());
                    num_bytes += in_current_file;
                    overflow_block = Some(block[in_current_file..].to_vec());
                    let bytes_written = f
                        .write_blocks(
                            offset - byte_range.start, 
                            &buf
                                .iter()
                                .map(|b| IoSlice::new(&b)).collect::<Vec<_>>()
                            )
                        .unwrap();
                    debug_assert_eq!(bytes_written, num_bytes);
                    buf.clear();
                    continue;
                }
                num_bytes += block.len();
                buf.push(block);
            }
    
            for (&block_offset, b) in self.blocks.iter() {
                let end_offset = block_offset + b.len();
                if end_offset > byte_range.end {
                    let overflow_size = end_offset - byte_range.end;
                    let in_current_file = b.len() - overflow_size;
                    buf.push(b[..in_current_file].to_vec());
                    num_bytes += in_current_file;
                    overflow_block = Some(b[in_current_file..].to_vec());
                    break;
                }
                buf.push(b.clone());
                num_bytes += b.len();
            }
    
            let bytes_written = f
                .write_blocks(
                    offset - byte_range.start, 
                    &buf
                        .iter()
                        .map(|b| IoSlice::new(&b)).collect::<Vec<_>>()
                    )
                .unwrap();
            debug_assert_eq!(bytes_written, num_bytes);
            offset += bytes_written as usize;
            buf.clear();
        }
    
        debug_assert_eq!(offset, piece_offset + self.length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_overlapping_blocks() {

        let piece = Piece {
            hash: [0; 20],
            length: 330_140,
            blocks: BTreeMap::new(),
            file_overlap: 1..8,
        };

        
    }
}