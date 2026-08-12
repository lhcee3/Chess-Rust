use once_cell::sync::Lazy;

const PIECE_TYPES: usize = 6;
const COLORS: usize = 2;
const SQUARES: usize = 64;

static ZOBRIST: Lazy<ZobristTables> = Lazy::new(|| ZobristTables::new());

pub struct ZobristTables {
    pub pieces: [[[u64; SQUARES]; PIECE_TYPES]; COLORS],
    pub castling: [u64; 4],
    pub en_passant: [u64; 8],
    pub side_to_move: u64,
}

impl ZobristTables {
    fn new() -> Self {
        let mut pieces = [[[0u64; SQUARES]; PIECE_TYPES]; COLORS];
        for c in 0..COLORS {
            for pt in 0..PIECE_TYPES {
                for sq in 0..SQUARES {
                    pieces[c][pt][sq] = rand::random();
                }
            }
        }
        let castling = [rand::random(), rand::random(), rand::random(), rand::random()];
        let mut en_passant = [0u64; 8];
        for f in 0..8 {
            en_passant[f] = rand::random();
        }
        let side_to_move = rand::random();
        ZobristTables { pieces, castling, en_passant, side_to_move }
    }
}

pub fn init_zobrist() {
    Lazy::force(&ZOBRIST);
}
