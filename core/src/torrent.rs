use std::{path::Path, fs::read};

use serde_derive::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
use bencode::{decode_bytes, encode_to_raw};

use crate::{Result, Error};

#[derive(Debug, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct File {

    // A list containing one or more string elements that together represent the path and filename
    path: Vec<String>,
    
    // Length of the file in bytes (integer)
    length: i64,
    
    // A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    md5sum: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Info {

    // File name
    name: String,
    
    // String consisting of the concatenation of all 20-byte SHA1 hash values, one per piece
    #[serde(with = "serde_bytes")]
    pieces: Vec<u8>,

    // Number of bytes in each piece (integer)
    #[serde(rename = "piece length")]
    piece_length: i64,

    // A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    md5sum: Option<String>,
    
    // Length of the file in bytes (integer)
    #[serde(default)]
    length: Option<i64>,

    // A list of dictionaries, one for each file
    #[serde(default)]
    files: Option<Vec<File>>,
    
    // If it is set to "1", the client MUST publish its presence to get other peers ONLY 
    // via the trackers explicitly described in the metainfo file. If this field is set to 
    // "0" or is not present, the client may obtain peer from other means, e.g. PEX peer exchange, dht
    #[serde(default)]
    private: Option<u8>,

    // A list containing one or more string elements that together represent the path and filename
    #[serde(default)]
    path: Option<Vec<String>>,

    #[serde(default)]
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Torrent {
    
    // The announce URL of the tracker (string)
    #[serde(default)]
    announce: String,
    
    // A dictionary that describes the file(s) of the torrent
    info: Info,
    
    // sha1 hash of info dict
    #[serde(skip)]
    info_hash: [u8; 20],

    #[serde(default)]
    nodes: Option<Vec<Node>>,
    
    // (optional) the string encoding format used to generate the pieces part of the info 
    // dictionary in the .torrent metafile (string)
    #[serde(default)]
    encoding: Option<String>,
    
    #[serde(default)]
    httpseeds: Option<Vec<String>>,
    
    // (optional) this is an extention to the official specification, offering backwards-compatibility
    #[serde(default)]
    #[serde(rename = "announce-list")]
    announce_list: Option<Vec<Vec<String>>>,
    
    // (optional) the creation time of the torrent, in standard UNIX epoch format
    #[serde(default)]
    #[serde(rename = "creation date")]
    creation_date: Option<i64>,
    
    // (optional) free-form textual comments of the author (string)
    #[serde(rename = "comment")]
    comment: Option<String>,
    
    // (optional) name and version of the program used to create the .torrent (string)
    #[serde(default)]
    #[serde(rename = "created by")]
    created_by: Option<String>,
}

impl Torrent {

    pub fn new(path: &Path) -> Result<Self> {

        let data = read(path)?;
        let mut torrent: Torrent = decode_bytes(&data)
            .map_err(
                |_| Error::BencodeError("failed to deserialize torrent".to_string())
            )?;
        
        torrent.info_hash = torrent.info.info_hash()?;
        
        Ok(torrent)
    }

    pub fn announce(&self) -> &str { &self.announce }

    pub fn encoding(&self) -> Option<&str> { self.encoding.as_deref() }

    pub fn name(&self) -> &str { &self.info.name }

    pub fn piece_length(&self) -> i64 { self.info.piece_length }

    
}

impl Info {
    // Calculates the sha1 hash of info dict to verify torrent integrity.
    fn info_hash(&self) -> Result<[u8; 20]> {
        let mut hasher = Sha1::new();
        // Serialize info dict into bencode.
        let info_data = encode_to_raw(&self)
            .map_err(|_| Error::BencodeError("failed to encode info dict".to_string())
        )?;
        hasher.update(info_data);
        Ok(hasher.finalize().into())
    }
}

#[cfg(test)]
mod test {
    
    use std::path::Path;
    use super::Torrent;
    use hex_literal::hex;

    #[test]
    fn new_torrent() {
        let p = Path::new("../test_torrents/test_single_file.torrent");
        let torrent = Torrent::new(&p).unwrap();

        assert_eq!(torrent.announce(), "http://linuxtracker.org:2710/00000000000000000000000000000000/announce");
        assert_eq!(torrent.encoding(), Some("UTF-8"));
        assert_eq!(torrent.name(), "backbox-6-desktop-amd64.iso");
        assert_eq!(torrent.piece_length(), 2097152);
        assert_eq!(torrent.info_hash[..], hex!("bd00ed1cf18e575a5cb829d4349bceed34d76833"));
    }

}