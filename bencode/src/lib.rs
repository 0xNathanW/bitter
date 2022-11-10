#![allow(unused)]

// Convention from https://serde.rs/conventions.html
mod encode;
mod decode;
mod error;
mod token;

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn load() {
        let b = std::fs::read(Path::new("../debian.torrent")).unwrap();
        println!("{:?}", String::from_utf8_lossy(&b));
    }
}
