use std::{collections::{HashMap, BTreeMap}, sync::{Arc, RwLock}};
use tokio::{sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle};
use crate::{store::StoreInfo, torrent::CommandToTorrent, block::{BlockInfo, num_blocks}};
use self::{piece::Piece, file::TorrentFile};

mod file;
mod piece;

pub type Result<T> = std::result::Result<T, DiskError>;

#[derive(thiserror::Error, Debug)]
pub enum DiskError {

    #[error(transparent)]
    IoError(#[from] std::io::Error),

}

pub enum CommandToDisk {

    // Block has been recieved and needs to be written to disk.
    WriteBlock {
        block: BlockInfo,
        data: Vec<u8>,
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

    disk_rx: UnboundedReceiver<CommandToDisk>,

    ctx: Arc<Ctx>,
    
}

#[derive(Debug)]
struct Ctx {

    pub files: Vec<RwLock<TorrentFile>>,
    
    pub torrent_tx: UnboundedSender<CommandToTorrent>,

}

pub async fn spawn(
    info: StoreInfo,
    piece_hashes: Vec<[u8; 20]>, 
    torrent_tx: UnboundedSender<CommandToTorrent>
) -> Result<(JoinHandle<Result<()>>, UnboundedSender<CommandToDisk>)> {
    tracing::info!("spawning disk task");
    let (mut disk, disk_tx) = Disk::new(info, piece_hashes, torrent_tx).await?;
    let handle = tokio::task::spawn(async move { disk.run().await });
    tracing::info!("disk task spawned");
    Ok((handle, disk_tx))
}

impl Disk {

    // TODO: Handle unwraps
    pub async fn new(info: StoreInfo, hashes: Vec<[u8; 20]>, torrent_tx: UnboundedSender<CommandToTorrent>) -> Result<(Self, UnboundedSender<CommandToDisk>)> {

        if !info.output_dir.is_dir() {
            std::fs::create_dir_all(&info.output_dir)?;
            tracing::info!("created missing output directory: {:?}", info.output_dir);
        }

        debug_assert!(info.files.len() > 0);
        let files = if info.files.len() == 1 {
            vec![RwLock::new(TorrentFile::new(&info.output_dir, info.files[0].clone())?)]
        } else {
            let mut files = Vec::new();
            for file in info.files.iter() {
                
                let path = info.output_dir.join(&file.path);
                
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
        
        let (disk_tx, disk_rx) = tokio::sync::mpsc::unbounded_channel();
        Ok((
            Self {
                info,
                piece_hashes: hashes,
                write_buf: HashMap::new(),
                disk_rx,
                ctx: Arc::new(Ctx {
                    files,
                    torrent_tx,
                })
            },
            disk_tx,
        ))
    }

    #[tracing::instrument(name = "disk", skip_all)]
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("starting disk loop");
        while let Some(cmd) = self.disk_rx.recv().await {
            match cmd {
                CommandToDisk::WriteBlock { block, data } => self.add_block(block, data)?,
                CommandToDisk::Shutdown => {
                    tracing::info!("disk shutdown");
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn add_block(&mut self, info: BlockInfo, data: Vec<u8>) -> Result<()> {
        
        let piece_idx = info.piece_idx;
        
        if !self.write_buf.contains_key(&piece_idx) {
            self.add_piece(piece_idx);
        }
        
        let piece = self.write_buf.get_mut(&piece_idx).unwrap();
        piece.add_block(info.offset, data);

        // If we have all the blocks for this piece, attempt to write to disk.
        if piece.blocks.len() == num_blocks(piece.length) as usize {
            
            let piece = self.write_buf.remove(&piece_idx).unwrap();
            let offset = self.info.piece_byte_offset(piece_idx);
            let ctx = Arc::clone(&self.ctx);

            tokio::task::spawn_blocking(move || {
                if piece.verify_hash() {
                    piece.write(offset, &ctx.files);
                    let _ = ctx.torrent_tx.send(CommandToTorrent::PieceWritten { idx: piece_idx, valid: true });
                } else {
                    tracing::warn!("piece {} failed hash verification", piece_idx);
                    let _ = ctx.torrent_tx.send(CommandToTorrent::PieceWritten { idx: piece_idx, valid: false });
                }
            });
        }
        Ok(())
    }

    pub fn add_piece(&mut self, piece_idx: usize) {
        let length = self.info.piece_length(piece_idx);
        let piece = Piece {
            hash: self.piece_hashes[piece_idx],
            length,
            blocks: BTreeMap::new(),
            file_overlap: self.info.piece_file_intersections(piece_idx),
        };
        self.write_buf.insert(piece_idx, piece);
    }
}

