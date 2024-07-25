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

// Very widely used, often cloned but is cheap.
#[derive(Debug, Clone)]
pub struct TorrentInfo {

    pub total_len: u64,

    pub piece_len: usize,

    pub last_piece_len: usize,

    pub num_pieces: u32,

}

impl TorrentInfo {

    pub fn new(metainfo: &MetaInfo) -> Self {
        
        let total_len = metainfo.total_len();
        let num_pieces = metainfo.num_pieces();
        let piece_len = metainfo.piece_len();
        let last_piece_len = (total_len - (piece_len as u64 * (num_pieces as u64 - 1))) as usize;

        Self {
            total_len,
            piece_len,
            last_piece_len,
            num_pieces,
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
}
