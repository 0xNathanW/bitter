use core::{metainfo::MetaInfo, torrent::{Torrent, TorrentConfig}};
use std::{path::Path, net::SocketAddr};

const DEFAULT_PORT: u16 = 6881;
const DEFAULT_CLIENT_ID: [u8; 20] = *b"-RS0133-73b3b0b0b0b0";

#[tokio::main]
async fn main() {
    let sub = tracing_subscriber::fmt::fmt().finish();
    tracing::subscriber::set_global_default(sub).unwrap();
    let meta_info = MetaInfo::new(Path::new("./test_torrents/test_single.torrent")).unwrap();
    let config = TorrentConfig {
        client_id: DEFAULT_CLIENT_ID,
        listen_address: SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), DEFAULT_PORT),
    };
    let mut torrent = Torrent::new(meta_info, config);
    torrent.start().await.unwrap();
}
