use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde::ser::{SerializeSeq, SerializeMap};
use serde::de;

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

impl<'de> Deserialize<'de> for Token {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_any(TokenVisitor)
    }
}

pub struct TokenVisitor;

impl<'de> de::Visitor<'de> for TokenVisitor {

    type Value = Token;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("any bencode token type")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where E: de::Error 
    {
        Ok(Token::Integer(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where E: de::Error
    {
        Ok(Token::Integer(v as i64))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where E: de::Error 
    {
        Ok(Token::ByteString(v.into()))    
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where E: de::Error 
    {
        Ok(Token::ByteString(v.into()))    
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where E: de::Error
    {
        Ok(Token::ByteString(v.into()))
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where E: de::Error 
    {
        Ok(Token::ByteString(v.into()))    
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where A: de::SeqAccess<'de>
    {
        let mut out = Vec::new();
        while let Some(elem) = seq.next_element()? {
            out.push(elem)
        }
        Ok(Token::List(out))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: de::MapAccess<'de> 
    {
        let mut hmap = HashMap::new();
        if let Some((k, v)) = map.next_entry()? {
            hmap.insert(k, v);
        }
        Ok(Token::Dictionary(hmap))
    }
}