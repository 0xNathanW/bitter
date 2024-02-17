use std::{fs, path, io::{Write, Seek}};
use crate::store::FileInfo;
use super::Result;

#[derive(Debug)]
pub struct TorrentFile {
    pub info: FileInfo,
    pub handle: fs::File,
}

impl TorrentFile {

    pub fn new(dir: &path::Path, info: FileInfo) -> Result<Self> {

        let path = dir.join(&info.path);
        tracing::info!("creating file: {:?}", &path);
        let handle = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;

        Ok(Self {
            info,
            handle,
        })
    }

    pub fn write_blocks(
        &mut self, 
        offset: usize,
        blocks: &[std::io::IoSlice<'_>],
    ) -> Result<usize> {
        let mut n = 0;
        self.handle.seek(std::io::SeekFrom::Start(offset as u64))?;
        n += self.handle.write_vectored(blocks)?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {

}
