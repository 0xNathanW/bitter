use bittorrent::{start_client, MetaInfo, UserCommand};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Set up logging.
    let format = tracing_subscriber::fmt::format();
    let sub = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .event_format(format)
        .finish();
    tracing::subscriber::set_global_default(sub).unwrap();

    // console_subscriber::init();

    let (client, mut rx) = start_client(None);
    
    // let metainfo = MetaInfo::new("bittorrent/tests/test_torrents/test_smol.torrent")?;
    // client.new_torrent(metainfo)?;
    
    let metainfo = MetaInfo::new("bittorrent/tests/test_torrents/test_multi.torrent")?;
    client.new_torrent(metainfo)?;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            UserCommand::TorrentFinished { id} => {
                // tracing::error!("torrent result {}: {:?}", hex::encode(id), result);
            },
            UserCommand::TorrentStats { id, stats } => {
                // stats.peer_stats.iter().for_each(|peer| {
                //     tracing::info!("peer: {:#?}", peer);
                // });
            },
        }
    }

    client.shutdown().await?;
    Ok(())
}
