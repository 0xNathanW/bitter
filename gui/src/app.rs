use core::torrent::Torrent;
use std::rc::Rc;
pub struct App {
    torrent: Rc<Torrent>,
}