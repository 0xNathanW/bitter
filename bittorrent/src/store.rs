use core::panic;
use std::{path::PathBuf, ops::Range};
use serde_derive::{Deserialize, Serialize};
use crate::metainfo::MetaInfo;

// File information deserialised from metainfo.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "File")]
pub struct FileInfo {

    // A list containing one or more string elements that together represent the path and filename
    #[serde(deserialize_with = "crate::de::path_deserialize")]
    pub path: PathBuf,
    
    // Length of the file in bytes (integer)
    pub length: usize,
    
    // Offset in bytes from start of torrent when viewed as single array.
    #[serde(skip)]
    pub offset: usize,

    // A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    pub md5sum: Option<String>,

}

impl FileInfo {
    // Byte index range for whole torrent.
    pub fn byte_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.length)
    }
}

// Contains general information on torrage storage.
#[derive(Debug, Clone)]
pub struct StoreInfo {

    // Length of torrent in bytes.
    pub total_len: u64,

    // Length of pieces in bytes.
    pub piece_len: usize,

    // Length of the last piece, will be < piece_length.
    pub last_piece_len: usize,

    // Number of pieces in torrent.
    pub num_pieces: u32,

    // File contained in torrent.
    pub files: Vec<FileInfo>,

    // Directory to store downloaded files.
    pub output_dir: PathBuf,

}

impl StoreInfo {

    pub fn new(metainfo: &MetaInfo, output_dir: PathBuf) -> Self {
        
        let total_len = metainfo.total_len();
        let num_pieces = metainfo.num_pieces();
        let piece_len = metainfo.piece_len();
        let last_piece_len = (total_len - (piece_len as u64 * (num_pieces as u64 - 1))) as usize;
        let files = metainfo.files();
        let output_dir = if metainfo.is_multi_file() {
            output_dir.join(metainfo.name())
        } else {
            output_dir
        };

        Self {
            total_len,
            piece_len,
            last_piece_len,
            num_pieces,
            files,
            output_dir,
        }
    }

    // Returns length of piece given its index.
    pub fn piece_len(&self, idx: usize) -> usize {
        if idx as u32 == self.num_pieces - 1 {
            self.last_piece_len
        } else {
            self.piece_len
        }
    }

    // Returns the total bytes offset within the torrent of a piece.
    pub fn piece_total_offset(&self, piece_idx: usize) -> usize {
        piece_idx * self.piece_len
    }

    // Returns the indexes of the first and last file that a piece intersects.
    pub fn piece_file_intersections(&self, piece_idx: usize) -> Range<usize> {
        debug_assert!(piece_idx < self.num_pieces as usize, "piece index out of bounds");

        // If only one file, there are no intersections to compute.
        if self.files.len() == 1 {
            return 0..1;
        }

        let offset = piece_idx * self.piece_len;
        let end = offset + self.piece_len(piece_idx) - 1;

        let start_file = match self.files
            .iter()
            .enumerate()
            .find(|(_, f)| f.byte_range().contains(&offset))
        {
            Some((idx, _)) => idx,
            None => panic!("piece byte offset exceeds file length"),   
        };

        let end_file = match self.files[start_file..]
            .iter()
            .enumerate()
            .find(|(_, f)| f.byte_range().contains(&end))
        {
            Some((idx, _)) => start_file + idx,
            None => panic!("piece last byte exceeds torrent length"),
        };

        start_file..(end_file + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_file_intersections() {
        let path = std::path::Path::new("tests/test_torrents/test_multi.torrent");
        let metainfo = MetaInfo::new(path).unwrap();
        let store_info = StoreInfo::new(&metainfo, std::path::PathBuf::from("freedom"));
        println!("{}", store_info.last_piece_len);
        for idx in 0..=8302 {
            assert_eq!(store_info.piece_file_intersections(idx), 0..1);
        }
        assert_eq!(store_info.piece_file_intersections(8303), 0..2);
        for idx in 8304..=11072 {
            assert_eq!(store_info.piece_file_intersections(idx), 1..2);
        }
        assert_eq!(store_info.piece_file_intersections(11073), 1..8);
    }

    #[test]
    fn test_piece_offset() {
        let metainfo = MetaInfo::new(std::path::Path::new("tests/test_torrents/test_multi.torrent")).unwrap();
        let store_info = StoreInfo::new(&metainfo, std::path::PathBuf::from("freedom"));
        assert_eq!(store_info.piece_total_offset(0), 0);
        assert_eq!(store_info.piece_total_offset(1), 524288);
        assert_eq!(store_info.piece_total_offset(11073), 5_805_441_024);
    }
}