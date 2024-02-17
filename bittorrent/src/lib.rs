#![allow(dead_code)]

mod metainfo;
mod store;
mod torrent;
mod tracker;
mod p2p;
mod fs;
mod block;
mod picker;
mod de;

const BLOCK_SIZE: usize = 0x4000;

type Bitfield = bitvec::vec::BitVec<u8, bitvec::order::Msb0>;

pub use metainfo::MetaInfo;
pub use torrent::{Torrent, TorrentConfig};