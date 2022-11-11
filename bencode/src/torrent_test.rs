#![allow(unused)]

use std::fs::read;
use std::path::Path;

use serde_bytes::ByteBuf;
use serde_derive::{self, Deserialize};

use crate::decode_bytes;

#[derive(Debug, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct File {
    path: Vec<String>,
    length: i64,
    #[serde(default)]
    md5sum: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Info {
    name: String,
    pieces: ByteBuf,
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

#[derive(Debug, Deserialize)]
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
    
    let torrent = read(Path::new("./test_torrents/test_single_file.torrent"))
        .expect("error reading file content");

    let out: Torrent = decode_bytes(&torrent).unwrap();

    assert_eq!(out.announce, "http://linuxtracker.org:2710/00000000000000000000000000000000/announce");
    assert_eq!(out.encoding, Some("UTF-8".to_string()));
    assert_eq!(out.info.name, "backbox-6-desktop-amd64.iso");
    assert_eq!(out.info.piece_length, 2097152);
    assert_eq!(out.info.files, None);
}