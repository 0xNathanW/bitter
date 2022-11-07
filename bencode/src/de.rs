use serde::de::Deserializer;
use serde::de;

use super::error::Error;
use super::TokenVisitor;

pub struct Decoder {

}

impl Deserializer for Decoder {

    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where V: serde::de::Visitor<'de> 
    {
        
    }
    

}

