use core::{metainfo::MetaInfo, torrent::Torrent};
use std::{path::Path, net::SocketAddr};

#[tokio::main]
async fn main() {
    let sub = tracing_subscriber::fmt::fmt().finish();
    tracing::subscriber::set_global_default(sub).unwrap();
    let meta_info = MetaInfo::new(Path::new("./test_torrents/test_single.torrent")).unwrap();
    let mut torrent = Torrent::new(&meta_info, [0; 20], SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), 0));
    torrent.start().await.unwrap();
}
