use rand::seq::SliceRandom;
use serde_derive::{Deserialize, Serialize};
use crate::{store::FileInfo, tracker::Tracker};

#[derive(Debug, thiserror::Error)]
pub enum MetaInfoError {

    #[error("bencode error whilst decoding metainfo: {0}")]
    BencodeError(#[from] bencode::Error),

    #[error("invalid file extension, expected .torrent")]
    InvalidExtension,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("invalid pieces length, must be divisible by 20")]
    InvalidPiecesLength,

    #[error("file(s) with size 0")]
    FileNoSize,

    #[error("file(s) with no path")]
    FileEmptyPath,

    #[error("file has absolute path")]
    FileAbsolutePath,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct File {

    // #[serde(deserialize_with = "crate::de::path_deserialize")]
    pub path: Vec<String>,

    pub length: u64,

    pub md5sum: Option<String>,

}

#[derive(Clone, Deserialize, Serialize)]
pub struct Info {

    // File namepub .
    pub name: String,
    
    // String consisting of the concatenation of all 20-byte SHA1 hash values, one per piece.
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,

    // Number of bytes in each piece (integer).
    #[serde(rename = "piece length")]
    pub piece_length: u32,

    // A 32-character hexadecimal string corresponding to the MD5 sum of the file.
    #[serde(default)]
    pub md5sum: Option<String>,
    
    // Length of the file in bytes (integer).
    #[serde(default)]
    pub length: Option<u64>,

    // A list of dictionaries, one for each file.
    #[serde(default)]
    pub files: Option<Vec<File>>,
    
    // If it is set to "1", the client MUST publish its presence to get other peers ONLY 
    // via the trackers explicitly described in the metainfo file. If this field is set to 
    // "0" or is not present, the client may obtain peer from other means, e.g. PEX peer exchange, dht.
    #[serde(default)]
    pub private: Option<u8>,

    #[serde(default)]
    #[serde(rename = "root hash")]
    pub root_hash: Option<String>,

}

impl Info {
    // Calculates the sha1 hash of info dict to verify torrent integrity.
    fn info_hash(&self) -> Result<[u8; 20], MetaInfoError> {
        use sha1::Digest;
        let mut hasher = sha1::Sha1::new();
        // Serialize info dict into bencode.
        let info_data = bencode::encode_to_raw(&self)?;
        hasher.update(info_data);
        Ok(hasher.finalize().into())
    }    
}

#[allow(dead_code)]
#[derive(Deserialize, Clone)]
pub struct MetaInfo {
    
    // The announce URL of the tracker (string).
    #[serde(deserialize_with = "crate::de::url_deserialize")]
    pub announce: url::Url,
    
    // A dictionary that describes the file(s) of the torrent.
    pub info: Info,
    
    // sha1 hash of info dict
    #[serde(skip)] 
    pub info_hash: [u8; 20],
    
    // (optional) the string encoding format used to generate the pieces part of the info 
    // dictionary in the .torrent metafile (string).
    #[serde(default)]
    pub encoding: Option<String>,
    
    // (optional) this is an extention to the official specification, offering backwards-compatibility.
    #[serde(default)]
    #[serde(rename = "announce-list")]
    #[serde(deserialize_with = "crate::de::announce_list_deserialize")]
    pub announce_list: Option<Vec<Vec<url::Url>>>,
    
    // (optional) the creation time of the torrent, in standard UNIX epoch format.
    #[serde(default)]
    #[serde(rename = "creation date")]
    pub creation_date: Option<i64>,
    
    // (optional) free-form textual comments of the author (string).
    #[serde(rename = "comment")]
    pub comment: Option<String>,
    
    // (optional) name and version of the program used to create the .torrent (string).
    #[serde(default)]
    #[serde(rename = "created by")]
    pub created_by: Option<String>,
    
}

impl MetaInfo {

    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<MetaInfo, MetaInfoError> {
        
        if path.as_ref().extension().unwrap_or_default() != "torrent" {
            return Err(MetaInfoError::InvalidExtension);
        }

        let mut metainfo: MetaInfo = bencode::decode_bytes(&std::fs::read(path)?)?;
        
        if metainfo.info.pieces.len() % 20 != 0 || metainfo.info.pieces.is_empty() {
            return Err(MetaInfoError::InvalidPiecesLength);
        }

        metainfo.info_hash = metainfo.info.info_hash()?;
        tracing::debug!("metainfo created: {:#?}", metainfo);
        Ok(metainfo)
    }

    pub fn piece_hashes(&self) -> Vec<[u8; 20]> {
        self.info.pieces
            .chunks_exact(20)
            // Safe as we have already checked length is a multiple of 20, in new.
            .map(|c| c.try_into().unwrap())
            .collect()
    }

    pub fn piece_len(&self) -> usize { self.info.piece_length as usize }

    pub fn num_pieces(&self) -> u32 { self.info.pieces.len() as u32 / 20 }

    pub fn is_multi_file(&self) -> bool { self.info.files.is_some() }
    
    pub fn single_file_len(&self) -> Option<u64> { self.info.length }

    pub fn total_len(&self) -> u64 {
        if let Some(files) = &self.info.files {
            files.iter().map(|f| f.length as u64).sum()
        } else {
            self.info.length.unwrap_or(0) as u64
        }
    }

    pub fn info_hash(&self) -> [u8; 20] { self.info_hash }
    
    pub fn name(&self) -> &str { &self.info.name }

    pub fn trackers(&self) -> Vec<Vec<Tracker>> {
        // If announce_list is present, we use that.
        if let Some(announce_list) = self.announce_list.clone() {
            let mut trackers = Vec::new();
            for mut tier in announce_list {
                let mut tier_trackers = Vec::new();
                // Randomly shuffle the trackers in the tier.
                tier.shuffle(&mut rand::thread_rng());
                for url in tier {
                    tier_trackers.push(Tracker::new(url));
                }
                trackers.push(tier_trackers);
            }
            trackers
        // Otherwise we just use the announce key.
        } else {
            vec![vec![Tracker::new(self.announce.clone())]]
        }
    }

    pub fn files(&self) -> Vec<FileInfo> {
        if let Some(files) = &self.info.files {
            let mut offset = 0;
            files.iter().map(|f| {
                let file_info = FileInfo {
                    path: f.path.join("/").into(),
                    length: f.length as usize,
                    offset,
                    md5sum: f.md5sum.clone(),
                };
                offset += f.length as usize;
                file_info
            }).collect()
        } else {
            vec![FileInfo {
                path: self.info.name.clone().into(),
                length: self.info.length.unwrap() as usize,
                offset: 0,
                md5sum: None,
            }]
        }
    }

    // Formatting methods.

    pub fn creation_date_fmt(&self) -> Option<String> {
        self.creation_date.map(|v| {
            let date = chrono::NaiveDateTime::from_timestamp_opt(v, 0);
            date.map(|v| v.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Invalid date".to_string())
        })
    }

    pub fn info_hash_hex(&self) -> String {
        hex::encode(&self.info_hash)
    }

    pub fn size_fmt(&self) -> String {
        if self.is_multi_file() {
            let size = self.info.files.as_ref().unwrap().iter()
                .map(|f| f.length)
                .sum::<u64>();
            format_size(size)
        } else {
            format_size(self.info.length.unwrap())
        }
    }
}

fn format_size(bytes: u64) -> String {
    let mut size = bytes as f64;
    let mut unit = "B";
    if size > 1024.0 {
        size /= 1024.0;
        unit = "KiB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "MiB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "GiB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "TiB";
    }
    format!("{:.2} {}", size, unit)
}

impl std::fmt::Debug for MetaInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetaInfo")
            .field("announce", &self.announce.as_str())
            .field("info", &self.info)
            .field("info_hash", &self.info_hash_hex())
            .field("encoding", &self.encoding)
            // Change urls to strings for printing.
            .field("announce_list", &self.announce_list.as_ref().map(|v| 
                v.iter().map(|v| 
                    v.iter().map(|v| 
                        v.as_str()
                    ).collect()
                ).collect::<Vec<Vec<&str>>>()
            ))
            .field("creation_date", &self.creation_date_fmt())
            .field("comment", &self.comment)
            .field("created_by", &self.created_by)
            .finish()
    }
}

// Dont want to print out the pieces field, so we implement Debug manually.
impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Info")
            .field("name", &self.name)
            .field("num pieces", &(&self.pieces.len() / 20))
            .field("piece_length", &self.piece_length)
            .field("md5sum", &self.md5sum)
            .field("length", &self.length)
            .field("files", &self.files)
            .field("private", &self.private)
            .field("root_hash", &self.root_hash)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metainfo() {
        // Test metainfo with small single file torrent.
        let metainfo = MetaInfo::new("tests/test_torrents/test_small.torrent").unwrap();
        assert_eq!(metainfo.num_pieces(), 1028);
        assert_eq!(metainfo.info.piece_length, 32_768);
        assert_eq!(metainfo.total_len(), 33_677_666);
        assert_eq!(metainfo.is_multi_file(), false);
        assert_eq!(metainfo.info_hash_hex(), "f1a8db22ffe20c7014c6267b5f68b97fdc438b1a");
    }

    #[test]
    fn debug_meta_info() {
        let metainfo = MetaInfo::new("tests/test_torrents/test_multi.torrent").unwrap();
        // Pretty debug print.
        println!("{:#?}", metainfo);
        println!("{}", metainfo.total_len());
    }
}
