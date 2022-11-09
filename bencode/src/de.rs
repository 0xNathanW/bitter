use std::io::Read;
use std::vec;

use serde::de::Deserializer;
use serde::{de, forward_to_deserialize_any};

use super::error::{Error, Result};
use super::token::TokenVisitor;

#[derive(PartialEq, Eq, Debug)]
enum DecodedType {
    Integer(i64),
    ByteString(Vec<u8>),
    List,
    Dictionary,
    EOF,
}

pub struct Access<'a, R: 'a + Read> {
    d:      &'a mut Decoder<R>,
    length: Option<usize>,
}

impl<'a, R: 'a + Read> Access<'a, R> {
    fn new(deserializer: &'a mut Decoder<R>, length: Option<usize>) -> Self {
        Self { d: deserializer, length }
    }
}

pub struct Decoder<R: Read> {
    scanner:    R,
    next_token: Option<DecodedType>,
}

impl<'de, R: Read> Decoder<R> {

    fn new(scanner: R) -> Self { Self { scanner, next_token: None } }

    fn read_next(&mut self) -> Result<DecodedType> { 
        if let Some(next) = self.next_token.take() {
            return Ok(next);
        }

        let mut buf = [0; 1];
        if self.scanner.read(&mut buf).map_err(Error::IoError)? != 1 {
            return Err(Error::EOF);
        }

        match buf[0] {
            b'i' => Ok(DecodedType::Integer(self.read_num(None)? as i64)),
            n @ b'0'..=b'9' => Ok(DecodedType::ByteString(self.read_bytes(n)?)),
            b'l' => Ok(DecodedType::List),
            b'd' => Ok(DecodedType::Dictionary),
            b'e' => Ok(DecodedType::EOF),
            d => Err(Error::InvalidToken(
                format!("invalid token: {}", d as char)
            )),
        }
    }

    fn read_num(&mut self, length: Option<u8>) -> Result<usize>{

        let mut buf = [0; 1];
        let mut out = vec![];
        if let Some(n) = length {
            out.push(n);
        }

        loop {
            // Case if a byte is not read.
            if self.scanner.read(&mut buf).map_err(Error::IoError)? != 1 {
                return Err(Error::EOF);
            }
            // Signals end of integer.
            if buf[0] == b'e' {
                let length_str = String::from_utf8(out).map_err(|_| Error::InvalidToken(
                        "attempted to convert non UTF_8 int to string during deserialization".to_string()
                    ))?;
                let length_int = length_str.parse().map_err(|_| Error::InvalidToken(
                    "cannot parse {} as an i64".to_string()
                ))?;
                return Ok(length_int);
            // Otherwise continue.
            } else {
                out.push(buf[0]);
            }
        }
    }

    fn read_bytes(&mut self, n: u8) -> Result<Vec<u8>> {
        
        let length = self.read_num(Some(n))?;
        let mut buf = vec![0u8; length];
        let n = self.scanner.read(&mut buf).map_err(Error::IoError)?;
        
        if n != length {
            Err(Error::EOF)
        } else {
            Ok(buf)
        }
    }
}

impl<'de, 'a, R: Read> Deserializer<'de> for &'a mut Decoder<R> {

    type Error = Error;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor<'de> 
    {
        match self.read_next()? {
            DecodedType::Integer(i) => visitor.visit_i64(i),
            DecodedType::ByteString(s) => visitor.visit_bytes(&s),
            DecodedType::List => visitor.visit_seq(Access::new(&mut self, None)),
            DecodedType::Dictionary => visitor.visit_map(Access::new(&mut self, None)),
            DecodedType::EOF => Err(Error::EOF),
        }
    }

    forward_to_deserialize_any! {
        bool char 
        i8 i16 i32 i64
        u8 u16 u32 u64
        f32 f64
        unit bytes byte_buf 
        seq map unit_struct tuple_struct
        ignored_any struct
    }

    fn deserialize_newtype_struct<V>(
            self,
            name: &'static str,
            visitor: V,
        ) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        visitor.visit_newtype_struct(self)    
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        visitor.visit_some(self)    
    }

    fn deserialize_enum<V>(
            self,
            name: &'static str,
            variants: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        visitor.visit_enum(Access::new(self, None))   
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        let b = self.read_next().and_then(
            |x| match x {
                DecodedType::ByteString(a) => Ok(a),
                _ => Err(Error::InvalidToken("expected bytes".to_string())),
            }
        )?;

        let s = std::str::from_utf8(&b).map_err(
            |_| Error::InvalidToken("expected bytes".to_string()),
        )?;
        visitor.visit_str(s)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        self.read_next().and_then(
            |x| match x {
                DecodedType::List => Ok(()),
                _ => Err(Error::InvalidToken("expected list".to_string())),
            }
        )?;
        visitor.visit_seq(Access::new(self, Some(len)))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de> {
        todo!()
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

        if self.length.is_some() {
            self.length.map(|mut n| n -= 1 );
            if self.length.unwrap() == 0 && self.d.read_next()? != DecodedType::EOF {
                return Err(Error::InvalidToken("expected e token".to_string()));
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
        if self.d.read_next()? != DecodedType::EOF {
            Err(Error::InvalidToken("expected e token".to_string()))
        } else {
            Ok(out)
        }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        let out = match self.d.read_next()? {
            DecodedType::List => visitor.visit_seq(Access::new(&mut *self.d, Some(len)))?,
            _ => return Err(Error::InvalidToken("expected list".to_string())),
        };
        if self.d.read_next()? != DecodedType::EOF {
            Err(Error::InvalidToken("expected e token".to_string()))
        } else {
            Ok(out)
        }
    }

    fn struct_variant<V>(
            self,
            fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value>
        where V: de::Visitor<'de> 
    {
        let out = Deserializer::deserialize_any(&mut *self.d, visitor)?;
        if self.d.read_next()? != DecodedType::EOF {
            Err(Error::InvalidToken("expected e token".to_string()))
        } else {
            Ok(out)
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
            
            e => Err(Error::InvalidToken(
                format!("expected bytes/map, got {:?}", e)
            )),
        }
    }
}

pub fn from_bytes<'de, T>(b: &'de [u8]) -> Result<T>
    where T: de::Deserialize<'de>
{
    de::Deserialize::deserialize(&mut Decoder::new(b))
}


pub fn from_str<'de, T>(s: &'de str) -> Result<T>
    where T: de::Deserialize<'de> 
{
    from_bytes(s.as_bytes())
}