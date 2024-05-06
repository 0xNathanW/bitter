use std::{collections::HashMap, path::{Path, PathBuf}};
use sha1::{Sha1, Digest};
use tokio::sync::mpsc;
use crate::{
    block::*, client::ClientCommand, disk::{spawn_disk, DiskCommand}, p2p::PeerCommand, store::TorrentInfo, torrent::TorrentCommand, Bitfield, MetaInfo, BLOCK_SIZE
};

const TEST_TORRENT_FILE_PATH: &str = "tests/test_torrents/test_multi.torrent";
const TEST_TORRENT_DIR_PATH: &str = "tests/test_data/";

// Tests reads by checking the existing pieces.
// Should return a full bitfield.
#[tokio::test]
#[ignore]
async fn test_disk_read() -> Result<(), Box<dyn std::error::Error>> {

    let metainfo = MetaInfo::new(Path::new(TEST_TORRENT_FILE_PATH))?;
    let id = metainfo.info_hash();
    let info = TorrentInfo::new(&metainfo);
    let piece_hashes = metainfo.piece_hashes();
    let num_pieces = metainfo.num_pieces() as usize;
    println!("num_pieces: {}", num_pieces);
    let dir: PathBuf = if metainfo.is_multi_file() {
        PathBuf::from(TEST_TORRENT_DIR_PATH).join(metainfo.info.name.clone())
    } else {
        PathBuf::from(TEST_TORRENT_DIR_PATH)
    };
    let files = if let Some(files) = metainfo.info.files {
        files
    } else {
        vec![crate::metainfo::File {
            path: vec![metainfo.info.name.clone()],
            length: metainfo.total_len(),
            md5sum: metainfo.info.md5sum,
        }]
    };
    
    
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let (torrent_tx, _) = mpsc::unbounded_channel();
    let (_, disk_tx) = spawn_disk(client_tx);
    disk_tx.send(DiskCommand::NewTorrent {
        id,
        info: info.clone(),
        piece_hashes: piece_hashes.clone(),
        torrent_tx,
        files,
        dir,
    })?;

    println!("checking existing pieces...");
    match client_rx.recv().await.expect("didn't recieve torrent allocation result") {
        ClientCommand::TorrentAllocation { id: _, res } => {
            assert_eq!(res.unwrap(), Bitfield::repeat(true, num_pieces));
        },
        _ => panic!("unexpected client command, expected allocation result"),
    }

    Ok(())
}

// This test writes the last piece to the disk and verifies that
// it writes the correct number of bytes to the files.
// Using last piece as it intersects all but the first file.
#[tokio::test]
#[ignore]
async fn test_disk_write() -> Result<(), Box<dyn std::error::Error>> {

    let metainfo = MetaInfo::new(Path::new(TEST_TORRENT_FILE_PATH))?;
    let id = metainfo.info_hash();
    let mut file_lens = HashMap::new();
    for file in metainfo.files() {
        file_lens.insert(file.path.clone(), file.length);
    }
    
    let info = TorrentInfo::new(&metainfo);
    let temp_dir = tempfile::TempDir::new_in(TEST_TORRENT_DIR_PATH)?;

    let last_piece_idx = metainfo.num_pieces() as usize - 1;
    let last_piece_len = info.piece_len(last_piece_idx);
    let num_blocks = num_blocks(last_piece_len) as usize;

    // Change last hash to reflect our data.
    let mut piece_hashes = metainfo.piece_hashes().to_vec();
    let mut hasher = Sha1::new();
    hasher.update(&vec![1; last_piece_len]);
    let last_hash: [u8; 20] = hasher.finalize().into();
    piece_hashes[last_piece_idx] = last_hash;

    let files = if let Some(files) = metainfo.info.files {
        files
    } else {
        vec![crate::metainfo::File {
            path: vec![metainfo.info.name.clone()],
            length: metainfo.total_len(),
            md5sum: metainfo.info.md5sum,
        }]
    };

    let (torrent_tx, mut torrent_rx) = mpsc::unbounded_channel();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let (_, disk_tx) = spawn_disk(client_tx);
    disk_tx.send(DiskCommand::NewTorrent {
        id,
        info,
        piece_hashes,
        torrent_tx,
        files,
        dir: temp_dir.path().to_path_buf(),
    })?;

    match client_rx.recv().await.expect("didn't recieve ") {
        ClientCommand::TorrentAllocation { id: _, res: _ } => {},
        _ => panic!("unexpected client command"),
    }

    // Write all blocks for the last piece.
    for i in 0..num_blocks {
        let block = Block {
            piece_idx: last_piece_idx,
            offset: i * BLOCK_SIZE as usize,
            data: BlockData::Owned(vec![1; block_len(last_piece_len, i)]),
        };
        disk_tx.send(DiskCommand::WriteBlock { id, block })?;
    }

    match torrent_rx.recv().await.expect("didn't recieve piece written cmd"){
        TorrentCommand::PieceWritten { idx, valid } => {
            assert_eq!(idx, last_piece_idx);
            assert!(valid);
        },
        _ => panic!("unexpected command"),
    }
    
    for file in std::fs::read_dir(temp_dir.path())? {
        let file = file?;
        let name = file.file_name();
        // First file not written to.
        if name == "CentOS-6.4-x86_64-bin-DVD1.iso" {
            continue;
        }
        let len = file_lens.get(&PathBuf::from(&name)).unwrap();
        let file_len = file.metadata()?.len();
        assert_eq!(file_len, *len as u64, "file {} length mismatch", name.to_string_lossy());
    }
    
    Ok(())
}