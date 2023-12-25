#![allow(dead_code)]

pub mod metainfo;
pub mod torrent;
pub mod tracker;
pub mod p2p;
pub mod fs;
pub mod block;
pub mod ctx;
pub mod picker;

pub const BLOCK_SIZE: u32 = 0x4000;

pub type Bitfield = bitvec::vec::BitVec<u8, bitvec::order::Msb0>;

