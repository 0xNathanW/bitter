use std::{io::Read, vec};
use serde::{
    de, 
    forward_to_deserialize_any,
    de::Deserializer,
};
use crate::error::{Error, Result};
use super::DecodedType;
use super::access::Access;

pub struct Decoder<R: Read> {
    pub scanner:    R,
    pub next_token: Option<DecodedType>,
}

impl<'de, R: Read> Decoder<R> {

    pub fn new(scanner: R) -> Self { Self { scanner, next_token: None } }

    pub fn read_next(&mut self) -> Result<DecodedType> { 
        if let Some(next) = self.next_token.take() {
            return Ok(next);
        }

        let mut buf = [0; 1];
        if self.scanner.read(&mut buf).map_err(Error::IoError)? != 1 {
            return Err(Error::EOF);
        }

        match buf[0] {
            b'i' => Ok(DecodedType::Integer(self.read_i64()?)),
            n @ b'0'..=b'9' => Ok(DecodedType::ByteString(self.read_bytes(n)?)),
            b'l' => Ok(DecodedType::List),
            b'd' => Ok(DecodedType::Dictionary),
            b'e' => Ok(DecodedType::EOF),
            e => Err(Error::InvalidToken { expected: "a valid token type".to_string(), found: (e as char).to_string() }),
        }
    }

    fn read_i64(&mut self) -> Result<i64>{

        let mut buf = [0; 1];
        let mut out = vec![];

        loop {
            // Case if a byte is not read.
            if self.scanner.read(&mut buf).map_err(Error::IoError)? != 1 {
                return Err(Error::EOF);
            }
            // Signals end of integer.
            if buf[0] == b'e' {
                
                let length_str = String::from_utf8(out).map_err(|err| Error::Custom(
                    format!("Failed to convert bytes to UTF-8 string: {}", err)                    
                ))?;
                
                let length_int = length_str.parse().map_err(|_| Error::Custom(
                    format!("cannot parse {} into int", length_str)
                ))?;

                return Ok(length_int);
            // Otherwise continue.
            } else {
                out.push(buf[0]);
            }
        }
    }

    fn read_usize(&mut self, n: u8) -> Result<usize> {

        let mut buf = [0; 1];
        let mut out = vec![n];

        loop {
            if self.scanner.read(&mut buf).map_err(Error::IoError)? != 1 {
                return Err(Error::EOF);
            }
            if buf[0] == b':' {

                let length_str = String::from_utf8(out).map_err(|err| Error::Custom(
                    format!("Failed to convert bytes to UTF-8 string: {}", err)                    
                ))?;
                
                let length_int = length_str.parse().map_err(|_| Error::Custom(
                    format!("cannot parse {} into int", length_str)
                ))?;

                return Ok(length_int);
            } else {
                out.push(buf[0]);
            }
        }
    }

    fn read_bytes(&mut self, n: u8) -> Result<Vec<u8>> {
        
        let length = self.read_usize(n)?;
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
            _name: &'static str,
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
            _name: &'static str,
            _variants: &'static [&'static str],
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
                _ => Err(Error::InvalidToken { expected: "b for byte string".to_string(), found: format!("{:?}", x) }),
            }
        )?;

        let s = std::str::from_utf8(&b).map_err(
            |err| Error::Custom(format!("Failed to convert bytes to UTF-8 string: {}", err))
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
                _ => Err(Error::InvalidToken { expected: "l for list".to_string(), found: format!("{:?}", x) }),
            }
        )?;
        visitor.visit_seq(Access::new(self, Some(len)))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where V: de::Visitor<'de> 
    {
        self.deserialize_str(visitor)
    }
}
