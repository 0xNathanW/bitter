use crate::Bitfield;

/*
A better strategy is to download pieces in rarest first order. The client can determine this
by keeping the initial bitfield from each peer, and updating it with every have message.
Then, the client can download the pieces that appear least frequently in these peer bitfields.
Note that any Rarest First strategy should include randomization among at least several of the
least common pieces, as having many clients all attempting to jump on the same "least common"
piece would be counter productive
*/

#[derive(Clone, Copy, Default, Debug)]
struct PieceInfo {
    // Number of peers that have this piece.
    frequency: usize,
    // Is the piece partially downloaded.
    is_partial: bool,
}

#[derive(Debug)]
pub struct Pieces {
    // All pieces in the torrent, idx aligned.
    pieces: Vec<PieceInfo>,
    // The pieces that we have.
    have: Bitfield,
}

impl Pieces {
    
    pub fn new(num_pieces: usize) -> Self {
        let mut have = Bitfield::new();
        have.resize(num_pieces, false);
        Self {
            pieces: vec![PieceInfo::default(); num_pieces],
            have,
        }
    }

    pub fn own_bitfield(&self) -> &Bitfield {
        &self.have
    }

    pub fn all(&self) -> bool {
        self.have.all()
    }
    
    pub fn set_own_bitfield(&mut self, bf: Bitfield) {
        debug_assert_eq!(bf.len(), self.have.len());
        self.have = bf;
    }

    pub fn increment_piece(&mut self, idx: usize) -> bool {
        assert!(idx < self.pieces.len());
        self.pieces[idx].frequency += 1;
        self.have[idx]
    }

    pub fn received_piece(&mut self, idx: usize) {
        assert!(idx < self.pieces.len());
        self.have.set(idx, true);
    }

    // Will return true if there is at least one piece that peer has and we don't.
    pub fn bitfield_update(&mut self, bf: &Bitfield) -> bool {
        debug_assert_eq!(bf.len(), self.have.len());
        let mut interested = false;
        bf
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .for_each(|(i, _)| {
                self.pieces[i].frequency += 1;
                if !self.have[i] {
                    interested = true;
                }
        });
        interested
    }

    pub fn pick_new_piece(&mut self, bf: &Bitfield) -> Option<usize> {
        for idx in 0..self.have.len() {
            let piece = &mut self.pieces[idx];
            if !self.have[idx] && piece.frequency > 0 && !piece.is_partial && bf[idx] {
                piece.is_partial = true;
                return Some(idx)
            }
        }
        None
    }
}