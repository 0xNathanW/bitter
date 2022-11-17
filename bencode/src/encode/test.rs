use std::collections::HashMap;
use serde::{Serialize, ser::SerializeStruct, Serializer};
use serde_derive::Serialize;

use super::encode_to_str;

#[test]
fn serialize_string() {
    let r = encode_to_str(&"foo").unwrap();
    assert_eq!(r, "3:foo")
}

#[test]
fn serialize_num() {
    let r = encode_to_str(&999).unwrap();
    assert_eq!(r, "i999e")
}

#[test]
fn serialize_vec() {
    let r = encode_to_str(&vec!["fooo", "bar"]).unwrap();
    assert_eq!(r, "l4:fooo3:bare")
}

struct TestStruct<'a> {
    a: &'a str,
    b: i64,
    c: Vec<u8>,
    d: HashMap<&'a str, Vec<u8>>,
}
impl Serialize for TestStruct<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer 
    {
           let mut state = serializer.serialize_struct("TestStrcut", 2)?;
           state.serialize_field("a", &self.a)?;
           state.serialize_field("b", &self.b)?;
           state.serialize_field("c", &self.c)?;
           state.serialize_field("d", &self.d)?;
           state.end()
    }
}
#[test]
fn test_serialization() {
    let mut s = TestStruct {
        a: "foo",
        b: 999,
        c: vec![1, 2, 3],
        d: HashMap::new()
    };
    s.d.insert("foo", vec![1, 2, 3]);
    s.d.insert("bar", vec![4, 5, 6]);
    let out = encode_to_str(&s).unwrap();
    assert_eq!(out, "d1:a3:foo1:bi999e1:cli1ei2ei3ee1:dd3:barli4ei5ei6ee3:fooli1ei2ei3eeee".to_string());
}

#[test]
fn serialize_struct() {
    #[derive(Debug, Serialize)]
    struct Fake {
        x: i64,
        y: String,
    }
    let f = Fake {
        x: 1111,
        y: "dog".to_string(),
    };
    assert_eq!(encode_to_str(&f).unwrap(), "d1:xi1111e1:y3:doge");
}

#[test]
fn serialize_lexical_sorted_keys() {
    #[derive(Serialize)]
    struct Fake {
        aaa: i32,
        bb: i32,
        z: i32,
        c: i32,
    }
    let f = Fake {
        aaa: 1,
        bb: 2,
        z: 3,
        c: 4,
    };
    assert_eq!(encode_to_str(&f).unwrap(), "d3:aaai1e2:bbi2e1:ci4e1:zi3ee");
}