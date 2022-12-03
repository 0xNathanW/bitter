use std::io::Read;
use serde::{Deserializer, de};

use crate::error::{Error, Result};
use super::decoder::Decoder;
use super::DecodedType;

pub struct Access<'a, R: 'a + Read> {
    d:      &'a mut Decoder<R>,
    length: Option<usize>,
}

impl<'a, R: 'a + Read> Access<'a, R> {
    pub fn new(deserializer: &'a mut Decoder<R>, length: Option<usize>) -> Self {
        Self { d: deserializer, length }
    }
}

impl<'de, 'a, R: 'a + Read> de::SeqAccess<'de> for Access<'a, R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
        where T: de::DeserializeSeed<'de> 
    {
        let out: Result<Option<T::Value>> = match self.d.read_next()? {
            DecodedType::EOF => Ok(None),
            x => {
                self.d.next_token = Some(x);
                Ok(Some(seed.deserialize(&mut *self.d)?))
            },
        };

        if let Some(l) = self.length {
            let l = l - 1;
            self.length = Some(l);
            if l == 0 && self.d.read_next()? != DecodedType::EOF {
                return Err(Error::InvalidType("expected e".to_string()))
            }
        }
        out
    }
}

impl<'de, 'a, R: Read> de::MapAccess<'de> for Access<'a, R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
        where K: de::DeserializeSeed<'de> 
    {
        match self.d.read_next()? {
            DecodedType::EOF => Ok(None),
            x => {
                self.d.next_token = Some(x);
                Ok(Some(seed.deserialize(&mut *self.d)?))
            },
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
        where V: de::DeserializeSeed<'de> 
    {
        seed.deserialize(&mut *self.d)
    }    
}

impl<'de, 'a, R: Read> de::VariantAccess<'de> for Access<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> { Ok(()) }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
        where T: de::DeserializeSeed<'de>
    {
        let out = seed.deserialize(&mut *self.d)?;
        match self.d.read_next()? {
            DecodedType::EOF => Ok(out),
            e => Err(Error::InvalidToken{ expected: "e for end".to_string(), found: format!("{:?}", e) }),
        }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        let out = match self.d.read_next()? {
            DecodedType::List => visitor.visit_seq(Access::new(&mut *self.d, Some(len)))?,
            e => return Err(Error::InvalidToken{ expected: "l for list".to_string(), found: format!("{:?}", e) }),
        };
        match self.d.read_next()? {
            DecodedType::EOF => Ok(out),
            e => Err(Error::InvalidToken{ expected: "e for end".to_string(), found: format!("{:?}", e) }),
        }
    }

    fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        let out = Deserializer::deserialize_any(&mut *self.d, visitor)?;
        match self.d.read_next()? {
            DecodedType::EOF => Ok(out),
            e => Err(Error::InvalidToken{ expected: "e for end".to_string(), found: format!("{:?}", e) }),
        }
    }
}

impl<'de, 'a, R: Read> de::EnumAccess<'de> for Access<'a, R> {
    type Error = Error;
    type Variant = Self;
    
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
        where V: de::DeserializeSeed<'de> 
    {
        match self.d.read_next()? {

            b @ DecodedType::ByteString(_) => {
                self.d.next_token = Some(b);
                Ok((seed.deserialize(&mut *self.d)?, self))
            },
            
            DecodedType::Dictionary => Ok((seed.deserialize(&mut *self.d)?, self)),
            
            e => Err(Error::InvalidToken{ expected: "b for bytes or d for dict".to_string(), found: format!("{:?}", e) }),
        }
    }
}