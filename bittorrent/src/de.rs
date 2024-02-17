use serde::{de, Deserialize};
use url::Url;
use crate::metainfo::MetaInfoError;

// Deserialiser functions for metainfo.

pub fn url_deserialize<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(de::Error::custom)
}

pub fn announce_list_deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<Vec<Url>>>, D::Error>
where
    D: de::Deserializer<'de>,
{  
    let raw = Vec::<Vec<String>>::deserialize(deserializer)?;
    let mut announce_list = Vec::new();
    
    for tier in raw {
        let mut urls = Vec::new();
        for url in tier {
            urls.push(Url::parse(&url).map_err(de::Error::custom)?);
        }
        announce_list.push(urls);
    }

    let total = announce_list.iter().map(|v| v.len()).sum::<usize>();
    if total == 0 { Ok(None) } else { Ok(Some(announce_list))}
}

pub fn path_deserialize<'de, D>(deserializer: D) -> Result<std::path::PathBuf, D::Error> 
where
    D: de::Deserializer<'de>
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    if raw.is_empty() {
        return Err(MetaInfoError::FileEmptyPath).map_err(de::Error::custom);
    }
    Ok(raw.into_iter().collect())
}
