use std::collections::HashMap;
use serde_derive::Deserialize;
use crate::token::Token;
use super::decode_str;

#[test]
fn decode_to_num() {
    let r: i64 = decode_str("i666e").unwrap();
    assert_eq!(r, 666);
}

#[test]
fn decode_to_string() {
    let r: String = decode_str("3:yes").unwrap();
    assert_eq!(r, "yes");
}

#[test]
fn decode_to_struct() {
    let b = "d1:xi1111e1:y3:dog1:z2:yoe";
    #[derive(PartialEq, Debug, Deserialize)]
    struct Fake {
        y: String,
        x: i64,
        #[serde(default)]
        z: Option<String>,
        #[serde(default)]
        a: Option<String>,
    }
    let r: Fake = decode_str(b).unwrap();
    assert_eq!(
        r,
        Fake {
            x: 1111,
            y: "dog".to_string(),
            z: Some("yo".to_string()),
            a: None,
        }
    );
}

#[test]
fn decode_to_map() {
    let b = "d1:xi1111e1:y3:doge";
    let r: Token = decode_str(b).unwrap();
    let mut d = HashMap::new();
    d.insert("x".into(), Token::Integer(1111_i64));
    d.insert("y".into(), Token::ByteString("dog".as_bytes().to_vec()));
    assert_eq!(r, Token::Dictionary(d));
}

#[test]
fn deserialize_to_vec() {
    let r: Vec<i64> = decode_str("li666ee").unwrap();
    assert_eq!(r, [666]);
}