use std::collections::HashMap;
use serde::Serialize;
use serde::ser::{SerializeSeq, SerializeMap};

// Bencode types.
pub enum Token {
    Integer(i64),
    ByteString(Vec<u8>),
    List(Vec<Token>),
    Dictionary(HashMap<Vec<u8>, Token>)
}

impl Serialize for Token {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer 
    {
        match self {
            Token::Integer(int) => serializer.serialize_i64(*int),

            Token::ByteString(string) => serializer.serialize_bytes(string),

            Token::List(list) => {
                let mut seq = serializer.serialize_seq(Some(list.len()))?;
                for elem in list {
                    seq.serialize_element(elem)?;
                }
                seq.end()
            },

            Token::Dictionary(dict) => {
                let mut map = serializer.serialize_map(Some(dict.len()))?;
                for (k, v) in dict {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            },
        }
    }
}