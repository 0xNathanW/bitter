use std::path::PathBuf;

#[derive(Debug)]
pub struct File {

    pub path: PathBuf,

    pub length: u64,

    pub offset: usize,

}
