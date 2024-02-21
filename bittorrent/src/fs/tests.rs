use std::{collections::HashMap, path::{Path, PathBuf}};
use sha1::{Sha1, Digest};
use crate::{
    block::*, 
    fs::{spawn, CommandToDisk}, 
    p2p::PeerCommand, 
    store::StoreInfo, 
    torrent::CommandToTorrent, 
    MetaInfo, 
    BLOCK_SIZE,
};

const TEST_TORRENT_FILE_PATH: &str = "tests/test_torrents/test_multi.torrent";
const TEST_TORRENT_DIR_PATH: &str = "tests/test_data/";

// Tests disk reads by reading blocks and thus pieces from completed torrent and verifying thier hashes.
// Need to set up a correct test torrent file and directory for this to work.
// Runs quite slow, probably due to the hashing.
#[tokio::test]
#[ignore]
async fn test_disk_read() -> Result<(), Box<dyn std::error::Error>> {

    let metainfo = MetaInfo::new(Path::new(TEST_TORRENT_FILE_PATH))?;
    let info = StoreInfo::new(&metainfo, TEST_TORRENT_DIR_PATH.into());
    let (torrent_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let (_h, disk_tx) = spawn(info.clone(), metainfo.piece_hashes(), torrent_tx).await?;
    let (peer_tx, mut peer_rx) = tokio::sync::mpsc::unbounded_channel();

    for piece_idx in 0..metainfo.num_pieces() as usize {

        let num_blocks = num_blocks(info.piece_len(piece_idx)) as usize;
        for block_idx in 0..num_blocks {
            let block_info = BlockInfo {
                piece_idx,
                offset: block_idx * BLOCK_SIZE as usize,
                len: block_len(info.piece_len(piece_idx), block_idx as usize),
            };
            disk_tx.send(CommandToDisk::ReadBlock { block: block_info, tx: peer_tx.clone() })?;
        }

        let mut piece_buf = vec![0; info.piece_len(piece_idx)];
        let mut blocks_received = 0;
        while let Some(cmd) = peer_rx.recv().await {
            match cmd {
                PeerCommand::BlockRead(block) => {
                    match block.data {
                        BlockData::Cached(data) => piece_buf[block.offset..block.offset + data.len()].copy_from_slice(&data),
                        _ => panic!("unexpected block data"),
                    }
                    blocks_received += 1;
                    if blocks_received == num_blocks {
                        break;
                    }
                }
                _ => panic!("unexpected command"),
            }
        }

        let mut hasher = Sha1::new();
        hasher.update(&piece_buf);
        let hash = hasher.finalize();
        assert_eq!(hash.as_slice(), metainfo.piece_hashes()[piece_idx], "piece {} hash mismatch", piece_idx);
        println!("piece {} read correctly", piece_idx);
    }

    Ok(())
}

// This test writes the last piece to the disk and verifies that
// it writes the correct number of bytes to the files.
// Using last byte as it intersects all but the first file.
#[tokio::test]
#[ignore]
async fn test_disk_write() -> Result<(), Box<dyn std::error::Error>> {

    let metainfo = MetaInfo::new(Path::new(TEST_TORRENT_FILE_PATH))?;
    let mut file_lens = HashMap::new();
    for file in metainfo.files() {
        file_lens.insert(file.path.clone(), file.length);
    }

    let temp_dir = tempfile::TempDir::new_in(TEST_TORRENT_DIR_PATH)?;
    let info = StoreInfo::new(&metainfo, temp_dir.path().into());

    let last_piece_idx = metainfo.num_pieces() as usize - 1;
    let last_piece_len = info.piece_len(last_piece_idx);
    let num_blocks = num_blocks(last_piece_len) as usize;

    // Change last hash to reflect our data.
    let mut piece_hashes = metainfo.piece_hashes().to_vec();
    let mut hasher = Sha1::new();
    hasher.update(&vec![1; last_piece_len]);
    let last_hash: [u8; 20] = hasher.finalize().into();
    piece_hashes[last_piece_idx] = last_hash;

    let (torrent_tx, mut torrent_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_h, disk_tx) = spawn(info.clone(), piece_hashes, torrent_tx).await?;

    for i in 0..num_blocks {
        let block_len = block_len(last_piece_len, i);
        let block = BlockInfo {
            piece_idx: last_piece_idx,
            offset: i * BLOCK_SIZE as usize,
            len: block_len,
        };
        let data = vec![1; block_len];
        disk_tx.send(CommandToDisk::WriteBlock { block, data })?;
    }

    let cmd = torrent_rx.recv().await;
    match cmd {
        Some(CommandToTorrent::PieceWritten { idx, valid }) => {
            assert_eq!(idx, last_piece_idx);
            assert!(valid);
        },
        _ => panic!("unexpected command"),
    }
    
    let sub_dir = std::fs::read_dir(temp_dir.path())?.next().unwrap()?;
    for file in std::fs::read_dir(sub_dir.path())? {
        let file = file?;
        let name = file.file_name();
        if name == "CentOS-6.4-x86_64-bin-DVD1.iso" {
            continue;
        }
        let len = file_lens.get(&PathBuf::from(&name)).unwrap();
        let file_len = file.metadata()?.len();
        assert_eq!(file_len, *len as u64, "file {} length mismatch", name.to_string_lossy());
    }
    
    Ok(())
}