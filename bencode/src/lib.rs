mod encode;
mod decode;
mod token;

// For bencode -> T
pub use decode::{decode_bytes, decode_str};

// For T -> bencode
pub use encode::{encode_to_raw, encode_to_str};

pub type Result<T> = std::result::Result<T, Error>;

// Errors specific to bencoding on top of those present in serde.
#[derive(Debug, thiserror::Error)]
pub enum Error {

    #[error(transparent)]
    IoError(std::io::Error),

    // Attempted to deserialize an invalid type.
    #[error("invalid type: {0}")]
    InvalidType(String),

    // Type valid but unexpected token.
    #[error("invalid token: expected: {expected:?}, found: {found:?}")]    
    InvalidToken {
        expected: String,
        found: String,
    },

    #[error("map serialization error: {0}")]
    MapSerializationOrder(String),

    #[error("{0}")]
    Custom(String),

    #[error("expected end of input stream")]
    EOF,

}

impl serde::ser::Error for Error {
    fn custom<T>(msg:T) -> Self where T:std::fmt::Display {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg:T) -> Self where T:std::fmt::Display {
        Error::Custom(msg.to_string())
    }
}
