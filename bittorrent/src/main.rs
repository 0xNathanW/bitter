use bittorrent::{start_client, CommandToUser, MetaInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Set up logging.
    let format = tracing_subscriber::fmt::format();
    let sub = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .event_format(format)
        .finish();
    tracing::subscriber::set_global_default(sub).unwrap();

    let (mut client, mut rx) = start_client(None)?;
    let metainfo = MetaInfo::new("bittorrent/tests/test_torrents/test_small.torrent").map_err(|e| {
        println!("failed to parse metainfo");
        e
    })?;
    client.new_torrent(metainfo, None)?;
    
    while let Some(cmd) = rx.recv().await {
        match cmd {
            CommandToUser::TorrentComplete(id) => {
                println!("torrent {} complete", hex::encode(id));
                break;
            },
            CommandToUser::TorrentError(e) => {
                println!("error: {}", e);
            },
            CommandToUser::TorrentStats { id: _, stats } => {
                // println!("stats: {:#?}", stats);
            }
        }
    }

    client.shutdown().await?;

    Ok(())
}
