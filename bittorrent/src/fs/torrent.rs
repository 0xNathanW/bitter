use std::{
    collections::HashMap, 
    io::{Read, Seek}, 
    sync::{Arc, Mutex, RwLock},
};
use crate::{
    block::{num_blocks, Block, BlockData}, 
    p2p::{PeerCommand, PeerTx}, 
    store::{FileInfo, StoreInfo}, 
    torrent::{CommandToTorrent, TorrentTx},
};
use super::{
    piece::Piece, 
    AllocationError, 
    BlockInfo, 
    DiskError,
};


#[derive(Debug)]
pub struct Torrent {

    info: StoreInfo,
    
    piece_hashes: Vec<[u8; 20]>,

    // Place to collect pieces, idxed by piece idx.
    write_buf: HashMap<usize, Piece>,

    // Context shared for piece writing task.
    ctx: Arc<Ctx>,
    
}

// Ctx involves data needed for the IO threads.
#[derive(Debug)]
struct Ctx {

    pub files: Vec<RwLock<TorrentFile>>,
    
    pub torrent_tx: TorrentTx,

    // Peers often will read multiple blocks from the same piece.
    // So we read whole piece thencache pieces to avoid disk syscalls.
    // Lru cache ensures least recently used pieces are removed.
    pub read_cache: Mutex<lru::LruCache<usize, Vec<Arc<Vec<u8>>>>>,

}


#[derive(Debug)]
pub struct TorrentFile {

    pub info:   FileInfo,

    pub handle: std::fs::File,

}

impl TorrentFile {
    pub fn new(dir: &std::path::Path, info: FileInfo) -> Result<Self, AllocationError> {       
        
        let path = dir.join(&info.path);
        tracing::info!("creating file: {:?}", &path);

        // Create and open the file with read/write permissions.
        let handle = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;

        Ok(Self { info, handle })
    }
}

impl Torrent {

    pub fn new(info: StoreInfo, piece_hashes: Vec<[u8; 20]>, torrent_tx: TorrentTx) -> Result<Self, AllocationError> {

        // Create output directories if they don't exist.
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
        
        // Unwrap safe because value is always > 0.
        // TODO: make this a config option?
        let read_cache = Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(500).unwrap()));
        Ok(Self {
            info,
            piece_hashes,
            write_buf: HashMap::new(),
            ctx: Arc::new(Ctx {
                files,
                torrent_tx,
                read_cache,
            })
        })
    }

    pub fn write_block(&mut self, block: Block) -> Result<(), DiskError> {
        tracing::trace!("write block offset {} in piece {}", block.offset, block.piece_idx);

        // Block info is validated in the peer session.
        let piece_idx = block.piece_idx;
        let piece = self.write_buf.entry(piece_idx).or_insert_with(|| {
            let length = self.info.piece_len(piece_idx);
            tracing::trace!("creating new piece {} in write buf", piece_idx);
            Piece {
                hash: self.piece_hashes[piece_idx],
                length,
                data: vec![0; length],
                blocks_received: vec![false; num_blocks(length) as usize],
                num_blocks_received: 0,
                file_overlap: self.info.piece_file_intersections(piece_idx),
            }
        });

        piece.add_block(&block);
        tracing::trace!("piece {}: {} blocks received out of {}", piece_idx, piece.num_blocks_received, num_blocks(piece.length));
        
        // If we have all the blocks for this piece, write piece to disk.
        if piece.num_blocks_received == num_blocks(piece.length) {
            tracing::trace!("all blocks received for piece {}", piece_idx);

            let piece = self.write_buf.remove(&piece_idx).expect("piece not found in write buf");
            let offset = piece_idx * self.info.piece_len;
            let ctx = Arc::clone(&self.ctx);

            // Spawn a thread to write the piece to disk.
            // TODO: maybe some error handling here on the message sends
            tokio::task::spawn_blocking(move || {

                if piece.verify_hash() {
                    tracing::trace!("piece {} hash verified, writing at offset {}", piece_idx, offset);
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

    // Reads a block from disk and sends it to the peer.
    pub fn read_block(&self, block_info: BlockInfo, peer_tx: PeerTx) -> Result<(), DiskError> {

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
            let piece_offset = block_info.piece_idx * self.info.piece_len;
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
