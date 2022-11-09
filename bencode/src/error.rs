use std::fmt::{Display, self};
use std::error::Error as StdError;
use std::result::Result as StdResult;

use serde::{ser, de};

pub type Result<T> = StdResult<T, Error>;

// Errors specific to bencoding on top of those present in serde.
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),

    // Attempted to deserialize an invalid type.
    InvalidType(String),

    // Type valid but unexpected token.
    InvalidToken(String),

    // Struct has an unknown field.
    UnknownField(String),

    // Struct field expected but missing.
    MissingField(String),

    MapSerializationOrder(String),

    Custom(String),

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

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_msg = match &self {
            Error::IoError(err) => return err.fmt(f),
            Error::InvalidType(s)           => s,
            Error::InvalidToken(s)          => s,
            Error::MissingField(s)          => s,
            Error::UnknownField(s)          => s,
            Error::Custom(s)                => s,
            Error::MapSerializationOrder(s) => s,
            Error::EOF                               => "end of stream"
        };
        f.write_str(error_msg)
    }
}