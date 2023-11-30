use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

// Errors specific to bencoding on top of those present in serde.
#[derive(Debug, thiserror::Error)]
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

impl serde::ser::Error for Error {
    fn custom<T>(msg:T) -> Self where T:Display {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg:T) -> Self where T:Display {
        Error::Custom(msg.to_string())
    }
}
