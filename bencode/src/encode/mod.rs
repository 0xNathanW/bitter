use serde::ser;

use crate::error::{Error, Result};

mod encoder;
mod map;
#[cfg(test)]
mod test;

pub fn encode_to_raw<T: ser::Serialize>(v: &T) -> Result<Vec<u8>> {
    let mut encoder = encoder::Encoder::new();
    v.serialize(&mut encoder)?;
    Ok(encoder.into_buf())
}

pub fn encode_to_string<T: ser::Serialize>(v: &T) -> Result<String> {
    let mut encoder = encoder::Encoder::new();
    v.serialize(&mut encoder)?;
    match std::str::from_utf8(encoder.as_ref()) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Err(Error::InvalidToken("not a utf-8".to_string())),
    }
}