use bittorrent::{start_client, UserCommand, MetaInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Set up logging.
    let format = tracing_subscriber::fmt::format();
    let sub = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .event_format(format)
        .finish();
    tracing::subscriber::set_global_default(sub).unwrap();

    let (mut client, mut rx) = start_client(None);
    let metainfo = MetaInfo::new("bittorrent/tests/test_torrents/test_single.torrent").map_err(|e| {
        println!("failed to parse metainfo");
        e
    })?;
    client.new_torrent(metainfo)?;
    
    while let Some(cmd) = rx.recv().await {
        match cmd {
            UserCommand::TorrentResult { id, result } => {
                println!("torrent result: {:?}", result);
            },
            UserCommand::TorrentStats { id: _, stats } => {
                // println!("stats: {:#?}", stats);
            },
            UserCommand::TrackerStats { id, stats } => {
                // println!("tracker stats");
            },
        }
    }

    client.shutdown().await?;

    Ok(())
}
