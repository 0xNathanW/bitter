use serde::de;
use crate::error::Result;

mod decoder;
mod access;
#[cfg(test)]
mod test;

use decoder::Decoder;

#[derive(PartialEq, Eq, Debug)]
pub enum DecodedType {
    Integer(i64),
    ByteString(Vec<u8>),
    List,
    Dictionary,
    EOF,
}

pub fn decode_bytes<'de, T>(b: &'de [u8]) -> Result<T>
    where T: de::Deserialize<'de>
{
    de::Deserialize::deserialize(&mut Decoder::new(b))
}


pub fn decode_str<'de, T>(s: &'de str) -> Result<T>
    where T: de::Deserialize<'de> 
{
    decode_bytes(s.as_bytes())
}