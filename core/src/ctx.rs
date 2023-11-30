use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use crate::{torrent::TorrentCommand, piece_selector::PieceSelector};

#[derive(Debug)]
pub struct TorrentContext {
    
    // The info hash for this torrent.
    pub info_hash:      [u8; 20],

    // The client ID for this client.
    pub client_id:      [u8; 20],

    pub piece_selector: Arc<RwLock<PieceSelector>>,

    // The number of pieces in this torrent.
    pub num_pieces:     usize,

    // The total size of this torrent in bytes.
    pub total_size:     u64,

    // Commands to the peer.
    pub cmd_tx:        UnboundedSender<TorrentCommand>,
    
}
