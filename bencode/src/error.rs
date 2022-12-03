use std::fmt::{Display};
use std::result::Result as StdResult;

use serde::{ser, de};
use thiserror::Error;

pub type Result<T> = StdResult<T, Error>;

// Errors specific to bencoding on top of those present in serde.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    IoError(std::io::Error),

    // Attempted to deserialize an invalid type.
    #[error("Invalid type: {0}")]
    InvalidType(String),

    // Type valid but unexpected token.
    #[error("Invalid token: expected: {expected:?}, found: {found:?}")]    
    InvalidToken {
        expected: String,
        found: String,
    },

    #[error("Map serialization error: {0}")]
    MapSerializationOrder(String),

    #[error("{0}")]
    Custom(String),

    #[error("Expected end of input stream")]
    EOF,
}

impl ser::Error for Error {
    fn custom<T>(msg:T) -> Self where T:Display {
        Error::Custom(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T>(msg:T) -> Self where T:Display {
        Error::Custom(msg.to_string())
    }
}
