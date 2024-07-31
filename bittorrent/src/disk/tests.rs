use crate::MetaInfo;



#[tokio::test]
#[ignore]
async fn test_disk_reads() -> Result<(), Box<dyn std::error::Error>> {

    let metainfo = MetaInfo::new("tests/test_torrents/test_multi.torrent")?;
    

    Ok(())
}