use serde::{de, Deserialize};
use serde_derive::{Deserialize, Serialize};
use bencode::{decode_bytes, encode_to_raw};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum MetaInfoError {

    #[error("bencode error: {0}")]
    BencodeError(#[from] bencode::Error),

    #[error("invalid file extension, expected .torrent")]
    InvalidExtension,

    #[error("error reading file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("invalid piece length, must be divisible by 20")]
    InvalidPieceLength,

    #[error("file has size 0")]
    FileNoSize,

    #[error("file has no path")]
    FileEmptyPath,
    
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct File {

    // A list containing one or more string elements that together represent the path and filename
    pub path: Vec<String>,
    
    // Length of the file in bytes (integer)
    pub length: u64,
    
    // A 32-character hexadecimal string corresponding to the MD5 sum of the file
    #[serde(default)]
    pub md5sum: Option<String>,

}

#[allow(dead_code)]
#[derive(Default, Deserialize, Serialize)]
pub struct Info {

    // File name.
    pub name: String,
    
    // String consisting of the concatenation of all 20-byte SHA1 hash values, one per piece.
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,

    // Number of bytes in each piece (integer).
    #[serde(rename = "piece length")]
    pub piece_length: u64,

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

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct MetaInfo {
    
    // The announce URL of the tracker (string).
    #[serde(deserialize_with = "url_deserialize")]
    pub announce: Url,
    
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
    #[serde(deserialize_with = "announce_list_deserialize")]
    pub announce_list: Option<Vec<Vec<Url>>>,
    
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

        let raw = std::fs::read(path)?;
        let mut metainfo: MetaInfo = decode_bytes(&raw)?;
        
        if metainfo.info.pieces.len() % 20 != 0 {
            return Err(MetaInfoError::InvalidPieceLength);
        }

        // Ensure that some file exists and have size, whether single file or multifile.
        if let Some(len) = metainfo.info.length {
            if len == 0 { return Err(MetaInfoError::FileNoSize) }
            
        } else if let Some(files) = &metainfo.info.files {
            for file in files {
                if file.path.is_empty() { return Err(MetaInfoError::FileEmptyPath) }
                else if file.length == 0 { return Err(MetaInfoError::FileEmptyPath) }
            }

        } else {
            return Err(MetaInfoError::FileEmptyPath);
        }

        metainfo.info_hash = metainfo.info.info_hash()?;
        tracing::debug!("MetaInfo created: {:#?}", metainfo);
        Ok(metainfo)
    }

    pub fn files(&self) -> Vec<crate::fs::File> {
        let mut files = vec![];
        if let Some(len) = self.info.length {
            files.push(crate::fs::File {
                path: std::path::PathBuf::from(self.info.name.clone()),
                length: len,
                offset: 0,
            })
        } else {
            let mut offset = 0;
            self.info.files.clone().unwrap().into_iter().for_each(|file| {
                files.push(crate::fs::File {
                    path: file.path.into_iter().collect(),
                    length: file.length,
                    offset,
                });
                offset += file.length as usize;
            });
        }
        files
    }

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

    pub fn num_pieces(&self) -> usize { self.info.pieces.len() / 20 }

    pub fn is_private(&self) -> bool { self.info.private == Some(1) }
    
    pub fn is_multi_file(&self) -> bool { self.info.files.is_some() }

    pub fn total_size(&self) -> u64 {
        if let Some(files) = &self.info.files {
            files.iter().map(|f| f.length).sum()
        } else {
            self.info.length.unwrap_or(0)
        }
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

impl Info {
    // Calculates the sha1 hash of info dict to verify torrent integrity.
    fn info_hash(&self) -> Result<[u8; 20], MetaInfoError> {
        use sha1::Digest;
        let mut hasher = sha1::Sha1::new();
        // Serialize info dict into bencode.
        let info_data = encode_to_raw(&self)?;
        hasher.update(info_data);
        Ok(hasher.finalize().into())
    }    
}

impl File {
    pub fn path(&self) -> String {
        self.path.join("/")
    }

    pub fn size_fmt(&self) -> String {
        format_size(self.length)
    }

    pub fn md5sum(&self) -> Option<&str> {
        self.md5sum.as_deref()
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

fn url_deserialize<'de, D>(deserializer: D) -> std::result::Result<Url, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(de::Error::custom)
}

fn announce_list_deserialize<'de, D>(deserializer: D) -> std::result::Result<Option<Vec<Vec<Url>>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    
    let list = Vec::<Vec<String>>::deserialize(deserializer)?;
    let mut result = Vec::new();
    
    for tier in list {
        let mut urls = Vec::new();
        for url in tier {
            urls.push(Url::parse(&url).map_err(de::Error::custom)?);
        }
        result.push(urls);
    }

    let total = result.iter().map(|v| v.len()).sum::<usize>();
    if total == 0 { Ok(None) } else { Ok(Some(result))}
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
            .field("num pieces", &self.pieces.len())
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
        let metainfo = MetaInfo::new("../test_torrents/test_multi.torrent").unwrap();
        println!("{:#?}", metainfo);
        println!("{:#?}", metainfo.files());
    }
}