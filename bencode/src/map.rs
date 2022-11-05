use std::mem;

use serde::{ser, Serializer};

use super::ser::Encoder;
use super::error::Error;

// A dictionary is encoded as d<contents>e. The elements of the dictionary are encoded with each key immediately 
// followed by its value. All keys must be byte strings and must appear in lexicographical order. A dictionary that associates 
// the values 42 and "spam" with the keys "foo" and "bar", respectively (in other words, {"bar": "spam", "foo": 42}), 
// would be encoded as follows: d3:bar4:spam3:fooi42ee.
pub struct SerializeMap<'a> {
    serializer:     &'a mut Encoder,
    items:          Vec<(Vec<u8>, Vec<u8>)>,
    current_key:    Option<Vec<u8>>,
}

impl<'a> SerializeMap<'a> {

    pub fn new(serializer: &'a mut Encoder, size: usize) -> Self {
        Self {
            serializer,
            items: Vec::with_capacity(size),
            current_key: None,
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.current_key.is_some() {
            return Err(Error::MapSerializationOrder(
                "attempted to end map serialization while holding key".to_string())
            )
        }
        // Take items and sort lexicographically.
        let mut items = mem::take(&mut self.items);
        items.sort_by(| k, v| {k.cmp(v)});
        
        self.serializer.push("d");
        for (k, v) in items {
            self.serializer.serialize_bytes(&k)?;
            self.serializer.push(v);
        }

        self.serializer.push("e");
        Ok(())
    }
}

impl<'a> ser::SerializeMap for SerializeMap<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
        where T: serde::Serialize 
    {
        match self.current_key {
            // We are supposed to be serializing value here.
            Some(_) => Err(Error::MapSerializationOrder("".to_string())),
            None => {
                let mut ser = Encoder::new();
                key.serialize(&mut ser);
                self.current_key = Some(ser.into_buf());
                Ok(())
            }
        }
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
        where T: serde::Serialize 
    {
        let key = self.current_key.take().ok_or_else(
            || { Error::InvalidToken("BingQiLing".to_string()) }
        )?;

        let mut ser = Encoder::new();
        value.serialize(&mut ser)?;
        let value = ser.into_buf();
        
        if !value.is_empty() { 
            self.items.push((key, value));
        }

        Ok(())
    }
    
    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error>
        where K: serde::Serialize, V: serde::Serialize, 
    {
        if self.current_key.is_some() {
            return Err(Error::MapSerializationOrder("".to_string()))
        }

        let mut key_ser = Encoder::new();
        key.serialize(&mut key_ser)?;
        let key = key_ser.into_buf();

        let mut val_ser = Encoder::new();
        value.serialize(&mut val_ser)?;
        let value = val_ser.into_buf();

        if !value.is_empty() {
            self.items.push((key, value));
        }
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> { self.finish() }
}

impl<'a> ser::SerializeStruct for SerializeMap<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
            &mut self,
            key: &'static str,
            value: &T,
        ) -> Result<(), Self::Error>
        where T: serde::Serialize 
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl<'a> ser::SerializeStructVariant for SerializeMap<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
            &mut self,
            key: &'static str,
            value: &T,
        ) -> Result<(), Self::Error>
        where T: serde::Serialize 
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish()?;
        self.serializer.push("e");
        Ok(())
    }
}