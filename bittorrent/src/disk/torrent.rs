use std::{
    collections::HashMap, 
    ops::Range, 
    path::PathBuf, 
    sync::{Arc, Mutex, RwLock},
};
use sha1::Digest;
use tokio::task::JoinHandle;
use crate::{
    block::{num_blocks, Block, BlockData},
    metainfo,
    p2p::{PeerCommand, PeerTx},
    info::TorrentInfo,
    torrent::{TorrentCommand, TorrentTx}, 
    Bitfield,
    ID,
};
use super::{
    piece::{read_piece, PieceBuf}, 
    AllocationError, 
    BlockRequest, 
    Result,
};


#[derive(Debug)]
pub struct Torrent {

    info: TorrentInfo,
    
    piece_hashes: Vec<ID>,

    // Place to collect pieces, idxed by piece idx.
    write_buf: HashMap<usize, PieceBuf>,

    // Context shared for piece writing task.
    ctx: Arc<Ctx>,
    
}

// Ctx involves data needed for the IO threads.
#[derive(Debug)]
struct Ctx {

    pub files: Vec<TorrentFile>,
    
    pub torrent_tx: TorrentTx,

    // Peers often will read multiple blocks from the same piece.
    // So we read whole piece thencache pieces to avoid disk syscalls.
    // Lru cache ensures least recently used pieces are removed.
    pub read_cache: Mutex<lru::LruCache<usize, Vec<Arc<Vec<u8>>>>>,

}


#[derive(Debug)]
pub struct TorrentFile {

    pub len:  usize,

    pub offset: usize,

    pub file_lock: RwLock<std::fs::File>,

    pub md5sum: Option<String>,

}

impl TorrentFile {
    pub fn byte_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.len)
    }
}

impl Torrent {

    pub fn new(
        files: Vec<metainfo::File>,
        dir: PathBuf,
        piece_hashes: Vec<ID>, 
        info: TorrentInfo,
        torrent_tx: TorrentTx,
    ) -> std::result::Result<Self, AllocationError> {

        // Create the output directory if it doesn't exist.
        if !dir.is_dir() {
            std::fs::create_dir_all(&dir)?;
            tracing::info!("created missing output directory: {:?}", dir);
        }

        debug_assert!(files.len() > 0);
        let mut offset = 0;
        let mut file_buf = Vec::with_capacity(files.len());
        for file in files.into_iter() {
            let path: PathBuf = file.path.join("/").into();
            let len = file.length as usize;
            // Create sub-directories if they don't exist.
            // TODO: handle more than one layer of subdirectories.
            if let Some(subdir) = path.parent() {
                if !subdir.exists() && subdir != PathBuf::from("") {
                    tracing::info!("creating sub-directory: {:?}", subdir);
                    std::fs::create_dir_all(&subdir)?;
                }
            }
            
            file_buf.push(
                    TorrentFile {
                        len,
                        offset,
                        file_lock: RwLock::new(
                            std::fs::OpenOptions::new()
                                .create(true)
                                .read(true)
                                .write(true)
                                .open(dir.join(&path))?,
                        ),
                        md5sum: file.md5sum,
                    }
            );
            tracing::info!("created file: {:?}", &dir.join(&path));
            offset += len;
        }

        // TODO: make lru size a configurable option?
        let read_cache = Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(500).unwrap()));
        Ok(Self {
            info,
            piece_hashes,
            write_buf: HashMap::new(),
            ctx: Arc::new(Ctx {
                files: file_buf,
                torrent_tx,
                read_cache,
            })
        })
    }

    pub fn write_block(&mut self, block: Block) {
        // Block info is validated in the peer session.

        let piece_idx = block.piece_idx;
        let piece = self.write_buf.entry(piece_idx).or_insert_with(|| {
            let len = self.info.piece_len(piece_idx);
            tracing::trace!("creating new piece {} in write buf", piece_idx);
            PieceBuf {
                hash: self.piece_hashes[piece_idx],
                len,
                data: vec![0; len],
                blocks_received: vec![false; num_blocks(len) as usize],
                num_blocks_received: 0,
                file_overlap: piece_file_intersections(&self.info, &self.ctx.files, piece_idx),
            }
        });

        piece.add_block(&block);
        tracing::trace!("piece {}: {} blocks received out of {}", piece_idx, piece.num_blocks_received, num_blocks(piece.len));
        
        // If we have all the blocks for this piece, write piece to disk.
        if piece.is_complete() {
            tracing::trace!("all blocks received for piece {} ... writing", piece_idx);

            let piece = self.write_buf.remove(&piece_idx).expect("piece not found in write buf");
            let offset = piece_idx * self.info.piece_len;
            let ctx = Arc::clone(&self.ctx);

            // Spawn a thread for expensive workload.
            // TODO: maybe some error handling here on the message sends
            let _: JoinHandle<Result<()>> = tokio::task::spawn_blocking(move || {

                if piece.verify_hash() {
                    piece.write(offset, &ctx.files)?;
                    ctx.torrent_tx.send(TorrentCommand::PieceWritten { idx: piece_idx, valid: true })?;
                } else {
                    tracing::warn!("piece {} failed hash verification", piece_idx);
                    ctx.torrent_tx.send(TorrentCommand::PieceWritten { idx: piece_idx, valid: false })?;
                }

                Ok(())
            });
        }
    }

    // Reads a block from disk and sends it to the peer.
    pub fn read_block(&self, block_info: BlockRequest, peer_tx: PeerTx) -> Result<()> {

        let block_idx = block_info.idx_in_piece();
        // If the block is in cache, retrieve it and send to peer.
        if let Some(cached) = self.ctx.read_cache.lock()?.get(&block_info.piece_idx) {
            tracing::trace!("cache hit for piece {}", block_info.piece_idx);
            
            if block_idx >= cached.len() {
                return Ok(());
            }
            
            peer_tx.send(PeerCommand::BlockRead(Block:: from_block_request(
                &block_info,
                BlockData::Cached(Arc::clone(&cached[block_idx])),
            ))).ok();
        
        } else {
            // If not in cache, read from disk and put in cache.
            let file_range = piece_file_intersections(&self.info, &self.ctx.files, block_info.piece_idx);
            let offset = block_info.piece_idx * self.info.piece_len;
            let len = self.info.piece_len(block_info.piece_idx);
            let ctx = Arc::clone(&self.ctx);

            // TODO: IDK if this is right?
            let _: JoinHandle<Result<()>> = tokio::task::spawn_blocking(move || {
                // TODO: Do we want to handle error, continuing task?
                let piece = read_piece(offset, len, &ctx.files[file_range])?;
                let block = Arc::clone(&piece[block_idx]);

                ctx.read_cache.lock()?.put(block_info.piece_idx, piece);
                peer_tx.send(PeerCommand::BlockRead(Block::from_block_request(
                    &block_info,
                    BlockData::Cached(block),
                ))).ok();
                Ok(())
            });
        }

        Ok(())
    }

    // Checks if the files exist, if so returns a bitfield of correctly occuring pieces.
    pub fn check_existing_files(&self) -> Bitfield {

        let mut bitfield = Bitfield::repeat(false, self.info.num_pieces as usize);
        
        // Iterate over all pieces and check hash matches.
        for piece_idx in 0..self.info.num_pieces as usize {
            let file_range = piece_file_intersections(&self.info, &self.ctx.files, piece_idx);
            match read_piece(
                piece_idx * self.info.piece_len,
                self.info.piece_len(piece_idx),
                &self.ctx.files[file_range],
            ) {
                Ok(piece) => {
                    let mut hasher = sha1::Sha1::new();
                    for block in piece.iter() {
                        hasher.update(&**block);
                    }
                    let hash = hasher.finalize();
                    if hash.as_slice() == self.piece_hashes[piece_idx] {
                        bitfield.set(piece_idx, true);
                    }
                },
                Err(_) => continue,
            }
        }

        bitfield
    }
}

// Returns the idxs of the first and last file that a piece intersects.
// TODO: kinda annoying this is not a method, but has to be, see if can fix.
pub fn piece_file_intersections(info: &TorrentInfo, files: &[TorrentFile], piece_idx: usize) -> Range<usize> {
    // If only one file, there are no intersections to compute.
    if files.len() == 1 {
        return 0..1;
    }

    let offset = piece_idx * info.piece_len;
    let end = offset + info.piece_len(piece_idx) - 1;

    let start_file = match files
        .iter()
        .enumerate()
        .find(|(_, f)| f.byte_range().contains(&offset))
    {
        Some((idx, _)) => idx,
        None => panic!("piece byte offset exceeds file length"),   
    };

    let end_file = match files[start_file..]
        .iter()
        .enumerate()
        .find(|(_, f)| f.byte_range().contains(&end))
    {
        Some((idx, _)) => start_file + idx,
        _ => panic!("piece last byte exceeds torrent length"),
    };

    start_file..(end_file + 1)
}