use std::{
    collections::HashMap, 
    io::{Read, Seek}, 
    sync::{Arc, Mutex, RwLock}
};
use tokio::{sync::mpsc, task};
use crate::{
    block::*, 
    store::{StoreInfo, FileInfo},
    p2p::{PeerCommand, PeerTx}, 
    torrent::{CommandToTorrent, TorrentTx},
};
use piece::Piece;

mod piece;
#[cfg(test)]
mod tests;

#[derive(thiserror::Error, Debug)]
pub enum DiskError {

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("sync error: {0}")]
    SyncError(String),

    #[error("channel error: {0}")]
    ChannelError(String),

}

impl<T> From<std::sync::PoisonError<T>> for DiskError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        DiskError::SyncError(e.to_string())
    }
}

impl<T> From<mpsc::error::SendError<T>> for DiskError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        DiskError::ChannelError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DiskError>;
pub type DiskTx = mpsc::UnboundedSender<CommandToDisk>;
pub type DiskRx = mpsc::UnboundedReceiver<CommandToDisk>;

pub enum CommandToDisk {

    // Block from peer needs to be written to disk.
    WriteBlock {
        block: BlockInfo,
        data: Vec<u8>,
    },

    // Block has been requested, needs to be read from disk.
    ReadBlock {
        block: BlockInfo,
        tx: PeerTx,
    },

    // Shutdown the disk task.
    Shutdown,

}

#[derive(Debug)]
pub struct Disk {

    // Storage information we need for writing to disk.
    info: StoreInfo,

    // Place to collect pieces.
    write_buf: HashMap<usize, Piece>,

    // Piece hashes.
    piece_hashes: Vec<[u8; 20]>,

    // Channel to receive commands from other tasks.
    disk_rx: DiskRx,

    // Context shared for piece writing task.
    ctx: Arc<Ctx>,
    
}

// Ctx involves data needed for the IO threads.
#[derive(Debug)]
struct Ctx {

    // Files to write to.
    pub files: Vec<RwLock<TorrentFile>>,
    
    // Channel to send commands to the torrent task.
    pub torrent_tx: TorrentTx,

    // Cached pieces to limit disk access.
    pub read_cache: Mutex<lru::LruCache<usize, Vec<Arc<Vec<u8>>>>>,

}

#[derive(Debug)]
pub struct TorrentFile {

    // Information about the file.
    pub info:   FileInfo,

    // File handle for access.
    pub handle: std::fs::File,

}

impl TorrentFile {
    pub fn new(dir: &std::path::Path, info: FileInfo) -> Result<Self> {       
        
        let path = dir.join(&info.path);
        tracing::info!("creating file: {:?}", &path);

        // Create and open the file with read/write permissions.
        let handle = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;

        Ok(Self {
            info,
            handle,
        })
    }
}

// Setup and spawn the disk task.
pub async fn spawn(
    info: StoreInfo,
    piece_hashes: Vec<[u8; 20]>,
    torrent_tx: TorrentTx,
) -> Result<(task::JoinHandle<Result<()>>, DiskTx)> {
    tracing::info!("spawning disk task");
    let (mut disk, disk_tx) = Disk::new(info, piece_hashes, torrent_tx).await?;
    let handle = tokio::task::spawn(async move { disk.run().await });
    tracing::info!("disk task spawned");
    Ok((handle, disk_tx))
}

impl Disk {

    pub async fn new(info: StoreInfo, piece_hashes: Vec<[u8; 20]>, torrent_tx: TorrentTx) -> Result<(Self, DiskTx)> {

        // Create output directory if it doesn't exist.
        if !info.output_dir.is_dir() {
            std::fs::create_dir_all(&info.output_dir)?;
            tracing::info!("created missing output directory: {:?}", info.output_dir);
        }

        // Create torrent files.
        debug_assert!(info.files.len() > 0);
        let files = if info.files.len() == 1 {
            vec![RwLock::new(TorrentFile::new(&info.output_dir, info.files[0].clone())?)]
        } else {
            let mut files = Vec::new();
            for file in info.files.iter() {                
                let path = info.output_dir.join(&file.path);
                // Create sub-directories if they don't exist.
                if let Some(subdir) = path.parent() {
                    if !subdir.exists() {
                        tracing::info!("creating sub-directory: {:?}", subdir);
                        std::fs::create_dir_all(&subdir)?;
                    }
                }
                files.push(RwLock::new(TorrentFile::new(&info.output_dir, file.clone())?));
            }
            files
        };
        
        let (disk_tx, disk_rx) = mpsc::unbounded_channel();
        // Unwrap safe because value is always > 0.
        let read_cache = Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(500).unwrap()));
        Ok((
            Self {
                info,
                piece_hashes,
                write_buf: HashMap::new(),
                disk_rx,
                ctx: Arc::new(Ctx {
                    files,
                    torrent_tx,
                    read_cache,
                })
            },
            disk_tx,
        ))
    }

    #[tracing::instrument(name = "disk", skip_all)]
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("starting disk");
        while let Some(cmd) = self.disk_rx.recv().await {
            match cmd {
                
                CommandToDisk::WriteBlock { block, data } => self.write_block(block, data)?,
                
                CommandToDisk::ReadBlock { block, tx } => self.read_block(block, tx)?,
                
                CommandToDisk::Shutdown => {
                    tracing::info!("disk shutdown");
                    break;
                },
            
            }
        }
        Ok(())
    }

    fn write_block(&mut self, info: BlockInfo, data: Vec<u8>) -> Result<()> {

        // Block info is validated in the peer session.
        tracing::trace!("received block: {:?}", info);
        let piece_idx = info.piece_idx;
        
        // If we don't have a piece for this block in the write buffer, create one.
        if !self.write_buf.contains_key(&piece_idx) {
            self.new_piece(piece_idx);
            tracing::trace!("added new piece {} to write buf", piece_idx);
        }
        // Add block to piece in write buffer.
        let piece = self.write_buf.get_mut(&piece_idx).expect("piece not found in write buf");
        piece.add_block(info.offset, data);

        tracing::trace!("piece {}: {} blocks received out of {}", piece_idx, piece.num_blocks_received, num_blocks(piece.length));
        // If we have all the blocks for this piece, write piece to disk.
        if piece.num_blocks_received == num_blocks(piece.length) {
            
            let piece = self.write_buf.remove(&piece_idx).expect("piece not found in write buf");
            let offset = self.info.piece_total_offset(piece_idx);
            let ctx = Arc::clone(&self.ctx);

            // Spawn a thread to write the piece to disk.
            // TODO: maybe some error handling here on the message sends
            tokio::task::spawn_blocking(move || {

                if piece.verify_hash() {
                    tracing::trace!("piece {} hash verified", piece_idx);
                    piece.write(offset, &ctx.files).unwrap();
                    ctx.torrent_tx.send(CommandToTorrent::PieceWritten { idx: piece_idx, valid: true }).map_err(|e| {
                        tracing::error!("channel failed to send piece written command to torrent: {}", e);
                        e
                    }).ok();
                
                } else {
                    tracing::warn!("piece {} failed hash verification", piece_idx);
                    ctx.torrent_tx.send(CommandToTorrent::PieceWritten { idx: piece_idx, valid: false }).map_err(|e| {
                        tracing::error!("channel failed to send piece written command to torrent: {}", e);
                        e
                    }).ok();
                
                }
            });
        }
        Ok(())
    }

    // Creates a new piece in the write buffer.
    fn new_piece(&mut self, piece_idx: usize) {
        let length = self.info.piece_len(piece_idx);
        let piece = Piece {
            hash: self.piece_hashes[piece_idx],
            length,
            data: vec![0; length],
            blocks_received: vec![false; num_blocks(length) as usize],
            num_blocks_received: 0,
            file_overlap: self.info.piece_file_intersections(piece_idx),
        };
        self.write_buf.insert(piece_idx, piece);
    }

    // Reads a block from disk and sends it to the peer.
    fn read_block(&self, block_info: BlockInfo, peer_tx: PeerTx) -> Result<()> {

        let block_idx = block_info.idx_in_piece();
        
        if let Some(cached) = self.ctx.read_cache.lock()?.get(&block_info.piece_idx) {
            tracing::trace!("cache hit for piece {}", block_info.piece_idx);
            
            if block_idx >= cached.len() {
                tracing::warn!("block index out of range");
                // Send a read error.
                return Ok(());
            }
            
            let block_data = Block {
                piece_idx: block_info.piece_idx,
                offset: block_info.offset,
                data: BlockData::Cached(cached[block_idx].clone()),
            };
            
            peer_tx.send(PeerCommand::BlockRead(block_data))?;
        
        } else {
            // If not in cache, read from disk and put in cache.
            let file_range = self.info.piece_file_intersections(block_info.piece_idx);
            let piece_offset = self.info.piece_total_offset(block_info.piece_idx);
            let piece_len = self.info.piece_len(block_info.piece_idx);
            let ctx = Arc::clone(&self.ctx);

            tokio::task::spawn_blocking(move || {
                let piece = read_piece(piece_offset, piece_len, file_range, &ctx.files[..]);
                let block = Arc::clone(&piece[block_idx]);

                ctx.read_cache.lock().unwrap().put(block_info.piece_idx, piece);

                peer_tx.send(PeerCommand::BlockRead(Block {
                    piece_idx: block_info.piece_idx,
                    offset: block_info.offset,
                    data: BlockData::Cached(block),
                })).map_err(|e| {
                    tracing::error!("failed to send block to peer: {}", e);
                    e
                }).ok();


            });
        }

        Ok(())
    }
}

fn read_piece(
    piece_offset: usize,
    piece_len: usize,
    file_range: std::ops::Range<usize>,
    files: &[RwLock<TorrentFile>],
) -> Vec<Arc<Vec<u8>>> {
    
    let mut bytes_read = 0;
    let mut total_offset = piece_offset;
    let mut buf = vec![0; piece_len];

    let files = &files[file_range];
    for file in files.iter() {
        let mut f = file.write().unwrap();
        
        let byte_range = f.info.byte_range();
        let file_offset = total_offset - byte_range.start;
        let piece_remaining = piece_len - bytes_read;
        let file_remaining = byte_range.end - total_offset;
        let bytes_remaining = std::cmp::min(piece_remaining, file_remaining);

        f.handle.seek(std::io::SeekFrom::Start(file_offset as u64)).unwrap();
        let n = f.handle.read(&mut buf[bytes_read..bytes_read + bytes_remaining]).unwrap();

        bytes_read += n;
        total_offset += n;
    }
    debug_assert_eq!(bytes_read, piece_len);
    
    buf.chunks(crate::BLOCK_SIZE as usize)
        .map(|chunk| Arc::new(chunk.to_vec()))
        .collect()
}
