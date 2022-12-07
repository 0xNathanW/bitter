use clap::Parser;
use core::{
    torrent::Torrent,
    tracker::tracker::Tracker,
};

#[derive(Parser)]
struct Args {
    #[arg(short, long, help = "Path to torrent file")]
    torrent: String,

    #[arg(short, long, help = "Verbose output")]
    verbose: bool,
}

#[tokio::main]
async fn main() {

    let args = Args::parse();

    let torrent_path = std::path::Path::new(&args.torrent);
    let torrent = Torrent::new(torrent_path).unwrap();
    let mut tracker = Tracker::new(&torrent);
    
    let peers = tracker.request_peers().await.unwrap();
    println!("{:#?}", peers);
}
