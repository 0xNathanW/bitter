use crate::BLOCK_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockData {
    // Index of piece that the block is contained in.
    pub piece_idx: usize,

    // Offset in bytes of block within piece.
    pub offset: usize,

    // Data of block.
    pub data: Vec<u8>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct BlockInfo {

    pub piece_idx: usize,

    pub offset: usize,

    pub len: usize,

}

impl BlockInfo {
    #[inline]
    pub fn idx_in_piece(&self) -> usize {
        self.offset / BLOCK_SIZE as usize
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
}
