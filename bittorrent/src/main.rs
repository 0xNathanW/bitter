use bittorrent::{MetaInfo, Torrent, TorrentConfig};
use std::{path::Path, net::SocketAddr};

const DEFAULT_PORT: u16 = 6881;
const DEFAULT_CLIENT_ID: [u8; 20] = *b"-RS0133-73b3b0b0b0b0";

#[tokio::main]
async fn main() {

    // Set up logging.
    let format = tracing_subscriber::fmt::format();
    let sub = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .event_format(format)
        .finish();
    tracing::subscriber::set_global_default(sub).unwrap();

    let metainfo = MetaInfo::new(Path::new("/home/nathan/Dev/bitter/bittorrent/tests/test_torrents/test_multi.torrent")).unwrap();
    let config = TorrentConfig {
        client_id: DEFAULT_CLIENT_ID,
        listen_address: SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), DEFAULT_PORT),
        min_max_peers: (5, 100),
        output_dir: "freedom".into(),
    };
    // assert!(config.output_dir.exists());
    let mut torrent = Torrent::new(metainfo, config).await;
    torrent.start().await.map_err(|e| tracing::error!("{}", e)).unwrap();
}
