use clap::Parser;
use core::{
    torrent::Torrent,
    tracker::tracker::Tracker,
    piece::PieceWorkQueue,
    p2p::parse_peers,
    p2p::peer::Peer,
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

    let work_queue = PieceWorkQueue::new(&torrent);
    
    let (peer_info, _active, _inactive) = tracker.request_peers().await.unwrap();
    let peers = parse_peers(peer_info);

    for peer in peers {
        let work_queue = work_queue.clone();
        tokio::spawn(async move {
            let mut peer = peer;
            peer.connect().await.unwrap();
            
 
        });

    }
}
