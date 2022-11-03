use std::collections::HashMap;

// Bencode types.
pub enum Token {
    
    Integer(i64),

    ByteString(Vec<u8>),
    
    List(Vec<Token>),
    
    Dictionary(HashMap<Vec<u8>, Token>),
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn load() {
        let b = std::fs::read(Path::new("../debian.torrent")).unwrap();
        println!("{:?}", String::from_utf8_lossy(&b));
    }
}
