use crate::BLOCK_SIZE;

pub struct Block {

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
    
    pub len: u32,

}

impl BlockInfo {
    pub fn idx_in_piece(&self) -> usize {
        self.offset / BLOCK_SIZE as usize
    }
}

pub(crate) fn block_size(piece_len: u32, block_idx: usize) -> u32 {
    BLOCK_SIZE.min(piece_len - (block_idx as u32 * BLOCK_SIZE))
}

pub(crate) fn num_blocks(piece_len: u32) -> u32 {
    (piece_len + (BLOCK_SIZE - 1)) / BLOCK_SIZE
}
