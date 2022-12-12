
pub enum Action {
    Request,
    Cancel,
}

pub struct Request {
    idx: u32,
    begin: u32,
    length: u32,
    action: Action,
}

impl Request {
    pub fn new(idx: u32, begin: u32, length: u32, action: Action) -> Self {
        Self { idx, begin, length, action }
    }
}