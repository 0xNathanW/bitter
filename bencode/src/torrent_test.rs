#![allow(unused)]

use std::fs::read;
use std::path::Path;

use serde_bytes::ByteBuf;
use serde_derive::{self, Deserialize, Serialize};

use crate::{decode_bytes, encode_to_str, encode_to_raw, Result};

#[derive(Debug, Deserialize, Serialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct File {
    path: Vec<String>,
    length: i64,
    #[serde(default)]
    md5sum: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    name: String,
    #[serde(with = "serde_bytes")]
    #[serde(skip)]
    pieces: Vec<u8>,
    #[serde(rename = "piece length")]
    piece_length: i64,
    #[serde(default)]
    md5sum: Option<String>,
    #[serde(default)]
    length: Option<i64>,
    #[serde(default)]
    files: Option<Vec<File>>,
    #[serde(default)]
    private: Option<u8>,
    #[serde(default)]
    path: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Torrent {
    #[serde(default)]
    announce: String,
    info: Info,
    #[serde(default)]
    nodes: Option<Vec<Node>>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    httpseeds: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    announce_list: Option<Vec<Vec<String>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    creation_date: Option<i64>,
    #[serde(rename = "comment")]
    comment: Option<String>,
    #[serde(default)]
    #[serde(rename = "created by")]
    created_by: Option<String>,
}

#[test]
fn single_file_torrent() {
    
    let torrent = read(Path::new("../test_torrents/test_single_file.torrent"))
        .expect("error reading file content");

    let out: Torrent = decode_bytes(&torrent).unwrap();

    assert_eq!(out.announce, "http://linuxtracker.org:2710/00000000000000000000000000000000/announce");
    assert_eq!(out.encoding, Some("UTF-8".to_string()));
    assert_eq!(out.info.name, "backbox-6-desktop-amd64.iso");
    assert_eq!(out.info.piece_length, 2097152);
    assert_eq!(out.info.files, None);
}

#[test]
fn test_parse_response() {
    let s = "64383a636f6d706c65746569376531303a696e636f6d706c657465693165383a696e74657276616c69313830306531323a6d696e20696e74657276616c693138303065353a706565727334383a52454d051ae161758649c35051ab05e8f6bb5062f69770469247493ad4d005879f2ec8d54237ce44ea6043db8806c8d565";
    let raw = hex::decode(s).unwrap();
    let readable = String::from_utf8_lossy(&raw);
    println!("{}", readable);
    let response: core::tracker::http_comms::TrackerResponse<core::tracker::peer_parse::BinaryModel> = decode_bytes(&raw).unwrap();
    println!("{:#?}", response);
}