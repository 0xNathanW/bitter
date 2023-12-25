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
    frequency: usize,
    is_partial: bool,
}

#[derive(Debug)]
pub struct PiecePicker {
    // All pieces in the torrent, idx aligned.
    pieces: Vec<PieceInfo>,
    // The pieces that we have.
    have: Bitfield,
}

impl PiecePicker {
    
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

    pub fn increment_piece(&mut self, piece_idx: usize) -> bool {
        debug_assert!(piece_idx < self.pieces.len());
        self.pieces[piece_idx].frequency += 1;
        self.have[piece_idx]
    }

    pub fn received_piece(&mut self, idx: usize) {
        self.have.set(idx, true);
    }

    pub fn bitfield_update(&mut self, bf: &Bitfield) -> bool {
        debug_assert_eq!(bf.len(), self.have.len());
        let mut interested = false;
        bf.iter().enumerate().for_each(|(i, b)| {
            if *b { 
                self.pieces[i].frequency += 1;
            }
            if !self.have[i] {
                interested = true;
            }
        });
        interested
    }

    pub fn pick_piece(&mut self) -> Option<usize> {
        for idx in 0..self.have.len() {
            let piece = &mut self.pieces[idx];
            if !self.have[idx] && piece.frequency > 0 && !piece.is_partial {
                piece.is_partial = true;
                return Some(idx)
            }
        }
        None
    }
}
