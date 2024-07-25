use crate::{info::TorrentInfo, BLOCK_SIZE};

// The data of a block can either be:
// 1. Owned - when peer sends us the data.
// 2. Cached - when we have the data cached and we need to share it with peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockData {
    Owned(Vec<u8>),
    Cached(std::sync::Arc<Vec<u8>>),
}

impl BlockData {

    pub fn len(&self) -> usize {
        match self {
            BlockData::Owned(data) => data.len(),
            BlockData::Cached(data) => data.len(),
        }
    }

    pub fn into_owned(self) -> Vec<u8> {
        match self {
            BlockData::Owned(data) => data,
            _ => panic!("cannot convert cached data to owned data"),
        }
    }
}

impl AsRef<[u8]> for BlockData {
    fn as_ref(&self) -> &[u8] {
        match self {
            BlockData::Owned(data) => data.as_ref(),
            BlockData::Cached(data) => data.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {

    // Index of piece that the block is contained in.
    pub piece_idx: usize,

    // Offset in bytes of block within piece.
    pub offset: usize,

    // Data of block.
    pub data: BlockData,

}

impl Block {
    pub fn from_block_request(info: &BlockRequest, data: BlockData) -> Self {
        Self {
            piece_idx: info.piece_idx,
            offset: info.offset,
            data,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct BlockRequest {

    pub piece_idx: usize,

    pub offset: usize,

    pub len: usize,

}

impl BlockRequest {
    pub fn from_block(block: &Block) -> Self {
        Self {
            piece_idx: block.piece_idx,
            offset: block.offset,
            len: block.data.len(),
        }
    }

    pub fn idx_in_piece(&self) -> usize {
        self.offset / BLOCK_SIZE as usize
    }

    pub fn is_valid(&self, info: &TorrentInfo) -> bool {
        if self.piece_idx >= info.num_pieces as usize {
            return false;
        }
        if self.offset + self.len > info.piece_len(self.piece_idx) {
            return false;
        }
        true
    }
}

pub fn block_len(piece_len: usize, block_idx: usize) -> usize {
    BLOCK_SIZE.min(piece_len - (block_idx * BLOCK_SIZE))
}

pub fn num_blocks(piece_len: usize) -> u32 {
    ((piece_len + (BLOCK_SIZE - 1)) / BLOCK_SIZE) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_len() {
        let normal_piece_len = 32_768;
        let last_piece_len = 24_930;
        let normal_block_len = 16_384;
        let last_block_len = 8546;
        assert_eq!(block_len(normal_piece_len, 0), normal_block_len);
        assert_eq!(block_len(normal_piece_len, 1), normal_block_len);
        assert_eq!(block_len(last_piece_len, 0), normal_block_len);
        assert_eq!(block_len(last_piece_len, 1), last_block_len);
    }

    #[test]
    fn test_num_blocks() {
        (0..12)
            .into_iter()
            .for_each(|i| assert_eq!(num_blocks(BLOCK_SIZE * i), i as u32));
        assert_eq!(num_blocks(BLOCK_SIZE + 500), 2);
        assert_eq!(num_blocks(BLOCK_SIZE * 5 + 1000), 6);
        assert_eq!(num_blocks(0), 0);
    }

    // TODO: add tests for BlockInfo::is_valid
}
