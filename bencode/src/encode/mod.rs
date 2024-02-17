use serde::ser;
use crate::{Error, Result};

mod encoder;
mod map;
mod string;

#[cfg(test)]
mod test;

pub fn encode_to_raw<T: ser::Serialize>(v: &T) -> Result<Vec<u8>> {
    let mut encoder = encoder::Encoder::new();
    v.serialize(&mut encoder)?;
    Ok(encoder.into_buf())
}

pub fn encode_to_str<T: ser::Serialize>(v: &T) -> Result<String> {
    let mut encoder = encoder::Encoder::new();
    v.serialize(&mut encoder)?;
    match std::str::from_utf8(encoder.as_ref()) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Err(Error::Custom("invalid utf-8 string".to_string())),
    }
}