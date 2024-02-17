use serde::ser;
use crate::{Error, Result};
use super::map::SerializeMap;

#[derive(Default)]
pub struct Encoder(Vec<u8>);

impl Encoder {
    pub fn new() -> Self { Self::default() }

    // Push tokens (a ref to u8 slice) to the internal buffer.
    pub fn push<T: AsRef<[u8]>>(&mut self, tokens: T) {
        self.0.extend_from_slice(tokens.as_ref());
    }

    // Returns ownership of underlying buf, consuming encoder.
    pub fn into_buf(self) -> Vec<u8> { self.0 }
}

impl AsRef<[u8]> for Encoder {
    fn as_ref(&self) -> &[u8] { &self.0 }
}

impl<'a> ser::Serializer for &'a mut Encoder {

    type Ok     = ();
    type Error  = Error;

    type SerializeSeq           = Self;
    type SerializeMap           = SerializeMap<'a>;
    type SerializeStruct        = SerializeMap<'a>;
    type SerializeStructVariant = SerializeMap<'a>;
    type SerializeTuple         = Self;
    type SerializeTupleStruct   = Self;
    type SerializeTupleVariant  = Self;

    // An integer is encoded as i<integer encoded in base ten ASCII>e. Leading zeros are not allowed (although the number 
    // zero is still represented as "0"). Negative values are encoded by prefixing the number with a hyphen-minus. The number 
    // 42 would thus be encoded as i42e, 0 as i0e, and -42 as i-42e. Negative zero is not permitted.

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.push("i");
        self.push(v.to_string());
        self.push("e");
        Ok(())
    }
    
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.push("i");
        self.push(v.to_string());
        self.push("e");
        Ok(())
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(Error::InvalidType("f32".to_string()))
    }

    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(Error::InvalidType("f64".to_string()))
    }

    // A byte string (a sequence of bytes, not necessarily characters) is encoded as <length>:<contents>. The length is 
    // encoded in base 10, like integers, but must be non-negative (zero is allowed); the contents are just the bytes that make 
    // up the string. The string "spam" would be encoded as 4:spam. The specification does not deal with encoding of 
    // characters outside the ASCII set; to mitigate this, some BitTorrent applications explicitly communicate the encoding (most 
    // commonly UTF-8) in various non-standard ways. This is identical to how netstrings work, except that netstrings 
    // additionally append a comma suffix after the byte sequence.

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.push(v.len().to_string());
        self.push(":");
        self.push(v);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0; 4];
        self.serialize_bytes(v.encode_utf8(&mut buf).as_bytes())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
        ) -> Result<()> 
    {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
            self,
            _name: &'static str,
            value: &T,
        ) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> { Ok(()) }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> { Ok(()) }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(self)    
    }

    fn serialize_none(self) -> Result<()> { Ok(()) }

    // A list of values is encoded as l<contents>e . The contents consist of the bencoded elements of the list, in order, 
    // concatenated. A list consisting of the string "spam" and the number 42 would be encoded as: l4:spami42ee. Note the 
    // absence of separators between elements, and the first character is the letter 'l', not digit '1'.

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.push("l");
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
            self,
            _name: &'static str,
            len: usize,
        ) -> Result<Self::SerializeTupleStruct> 
    {
        self.serialize_seq(Some(len))    
    }

    // A dictionary is encoded as d<contents>e. The elements of the dictionary are encoded with each key immediately 
    // followed by its value. All keys must be byte strings and must appear in lexicographical order. A dictionary that associates 
    // the values 42 and "spam" with the keys "foo" and "bar", respectively (in other words, {"bar": "spam", "foo": 42}), 
    // would be encoded as follows: d3:bar4:spam3:fooi42ee.

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(SerializeMap::new(self, len.unwrap_or(0)))
    }

    fn serialize_struct(
            self,
            _name: &'static str,
            len: usize,
        ) -> Result<Self::SerializeStruct> 
    {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            len: usize,
        ) -> Result<Self::SerializeStructVariant> 
    {
        self.push("d");
        self.serialize_bytes(variant.as_bytes())?;
        Ok(SerializeMap::new(self, len))
    }

    fn serialize_newtype_variant<T: ?Sized>(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            value: &T,
        ) -> Result<()>
        where T: serde::Serialize 
    {
        self.push("d");
        self.serialize_bytes(variant.as_bytes())?;
        value.serialize(&mut *self)?;
        self.push("e");
        Ok(())
    }

    fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant> 
    {
        self.push("d");
        self.serialize_bytes(variant.as_bytes())?;
        self.push("l");
        Ok(self)    
    }
}

impl ser::SerializeSeq for &mut Encoder {

    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(&mut **self)
    }

    // Bencode ends sequences with "e".
    fn end(self) -> Result<()> {
        self.push("e");
        Ok(())
    }
}

impl ser::SerializeTuple for &mut Encoder {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for &mut Encoder {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for &mut Encoder {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
        where T: serde::Serialize 
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.push("ee");
        Ok(())
    }
}


