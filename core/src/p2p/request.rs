
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