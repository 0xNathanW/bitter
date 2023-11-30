
pub struct Block {

    // Index of piece that the block is contained in.
    pub piece_idx: u32,

    // Offset in bytes of block within piece.
    pub offset: u32,

    // Data of block.
    pub data: Vec<u8>,

}

#[derive(Debug, Hash)]
pub struct BlockInfo {
    piece_idx: u32,
    offset: u32,
    len: usize,
}