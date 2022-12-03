// Convention from https://serde.rs/conventions.html
mod encode;
mod decode;
mod error;
mod token;

#[cfg(test)]
mod torrent_test;

// For bencode -> T
pub use decode::{decode_bytes, decode_str};

// For T -> bencode
pub use encode::{encode_to_raw, encode_to_str};

pub use error::{Error, Result};