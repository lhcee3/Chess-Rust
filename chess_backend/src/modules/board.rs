#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceType {
    pub fn to_char(self) -> char {
        match self {
            PieceType::Pawn => 'P',
            PieceType::Knight => 'N',
            PieceType::Bishop => 'B',
            PieceType::Rook => 'R',
            PieceType::Queen => 'Q',
            PieceType::King => 'K',
        }
    }

    pub fn from_char(c: char) -> Option<PieceType> {
        match c.to_ascii_uppercase() {
            'P' => Some(PieceType::Pawn),
            'N' => Some(PieceType::Knight),
            'B' => Some(PieceType::Bishop),
            'R' => Some(PieceType::Rook),
            'Q' => Some(PieceType::Queen),
            'K' => Some(PieceType::King),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub piece_type: PieceType,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
    pub piece: PieceType,
    pub captured: Option<PieceType>,
    pub promotion: Option<PieceType>,
    pub is_castle: bool,
    pub is_en_passant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub fn all() -> Self {
        CastlingRights {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }

    pub fn none() -> Self {
        CastlingRights {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }

    pub fn to_fen_string(&self) -> String {
        let mut s = String::new();
        if self.white_kingside { s.push('K'); }
        if self.white_queenside { s.push('Q'); }
        if self.black_kingside { s.push('k'); }
        if self.black_queenside { s.push('q'); }
        if s.is_empty() { s.push('-'); }
        s
    }
}

pub fn square_index(rank: usize, file: usize) -> usize {
    rank * 8 + file
}

pub fn rank_and_file(square: usize) -> (usize, usize) {
    (square / 8, square % 8)
}

pub fn square_to_algebraic(square: usize) -> String {
    let (rank, file) = rank_and_file(square);
    let file_char = (b'a' + file as u8) as char;
    let rank_char = (b'1' + rank as u8) as char;
    format!("{}{}", file_char, rank_char)
}

pub fn algebraic_to_square(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.len() != 2 {
        return None;
    }
    let chars: Vec<char> = s.chars().collect();
    let file = (chars[0] as u8).wrapping_sub(b'a');
    let rank = (chars[1] as u8).wrapping_sub(b'1');
    if file < 8 && rank < 8 {
        Some(square_index(rank as usize, file as usize))
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub pieces: [Option<Piece>; 64],
}

impl Board {
    pub fn new() -> Self {
        let mut pieces = [None; 64];

        pieces[0] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });
        pieces[1] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        pieces[2] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        pieces[3] = Some(Piece { piece_type: PieceType::Queen, color: Color::White });
        pieces[4] = Some(Piece { piece_type: PieceType::King, color: Color::White });
        pieces[5] = Some(Piece { piece_type: PieceType::Bishop, color: Color::White });
        pieces[6] = Some(Piece { piece_type: PieceType::Knight, color: Color::White });
        pieces[7] = Some(Piece { piece_type: PieceType::Rook, color: Color::White });
        for f in 0..8 {
            pieces[8 + f] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        }
        for f in 0..8 {
            pieces[48 + f] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        }
        pieces[56] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });
        pieces[57] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        pieces[58] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        pieces[59] = Some(Piece { piece_type: PieceType::Queen, color: Color::Black });
        pieces[60] = Some(Piece { piece_type: PieceType::King, color: Color::Black });
        pieces[61] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        pieces[62] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        pieces[63] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });

        Board { pieces }
    }

    pub fn get_piece(&self, rank: usize, file: usize) -> Option<Piece> {
        if rank < 8 && file < 8 {
            self.pieces[rank * 8 + file]
        } else {
            None
        }
    }

    pub fn set_piece(&mut self, rank: usize, file: usize, piece: Option<Piece>) {
        if rank < 8 && file < 8 {
            self.pieces[rank * 8 + file] = piece;
        }
    }

    pub fn get_piece_at(&self, square: usize) -> Option<Piece> {
        if square < 64 {
            self.pieces[square]
        } else {
            None
        }
    }

    pub fn set_piece_at(&mut self, square: usize, piece: Option<Piece>) {
        if square < 64 {
            self.pieces[square] = piece;
        }
    }

    pub fn is_square_attacked(&self, square: usize, by: Color) -> bool {
        let (rank, file) = rank_and_file(square);

        let pawn_dir: i8 = if by == Color::White { -1 } else { 1 };
        for &df in &[-1i8, 1i8] {
            let pr = rank as i8 + pawn_dir;
            let pf = file as i8 + df;
            if pr >= 0 && pr < 8 && pf >= 0 && pf < 8 {
                let idx = square_index(pr as usize, pf as usize);
                if let Some(p) = self.pieces[idx] {
                    if p.color == by && p.piece_type == PieceType::Pawn {
                        return true;
                    }
                }
            }
        }

        let knight_offsets: [(i8, i8); 8] = [
            (-2, -1), (-2, 1), (-1, -2), (-1, 2),
            (1, -2), (1, 2), (2, -1), (2, 1),
        ];
        for &(dr, df) in &knight_offsets {
            let kr = rank as i8 + dr;
            let kf = file as i8 + df;
            if kr >= 0 && kr < 8 && kf >= 0 && kf < 8 {
                let idx = square_index(kr as usize, kf as usize);
                if let Some(p) = self.pieces[idx] {
                    if p.color == by && p.piece_type == PieceType::Knight {
                        return true;
                    }
                }
            }
        }

        for dr in -1i8..=1 {
            for df in -1i8..=1 {
                if dr == 0 && df == 0 {
                    continue;
                }
                let kr = rank as i8 + dr;
                let kf = file as i8 + df;
                if kr >= 0 && kr < 8 && kf >= 0 && kf < 8 {
                    let idx = square_index(kr as usize, kf as usize);
                    if let Some(p) = self.pieces[idx] {
                        if p.color == by && p.piece_type == PieceType::King {
                            return true;
                        }
                    }
                }
            }
        }

        let straight_dirs: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for &(dr, df) in &straight_dirs {
            let mut cr = rank as i8 + dr;
            let mut cf = file as i8 + df;
            while cr >= 0 && cr < 8 && cf >= 0 && cf < 8 {
                let idx = square_index(cr as usize, cf as usize);
                if let Some(p) = self.pieces[idx] {
                    if p.color == by && (p.piece_type == PieceType::Rook || p.piece_type == PieceType::Queen) {
                        return true;
                    }
                    break;
                }
                cr += dr;
                cf += df;
            }
        }

        let diag_dirs: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
        for &(dr, df) in &diag_dirs {
            let mut cr = rank as i8 + dr;
            let mut cf = file as i8 + df;
            while cr >= 0 && cr < 8 && cf >= 0 && cf < 8 {
                let idx = square_index(cr as usize, cf as usize);
                if let Some(p) = self.pieces[idx] {
                    if p.color == by && (p.piece_type == PieceType::Bishop || p.piece_type == PieceType::Queen) {
                        return true;
                    }
                    break;
                }
                cr += dr;
                cf += df;
            }
        }

        false
    }

    pub fn display(&self) -> String {
        let mut s = String::new();
        for rank in (0..8).rev() {
            for file in 0..8 {
                let piece = self.get_piece(rank, file);
                let c = match piece {
                    Some(Piece { piece_type, color }) => {
                        let base = match piece_type {
                            PieceType::Pawn => 'P',
                            PieceType::Knight => 'N',
                            PieceType::Bishop => 'B',
                            PieceType::Rook => 'R',
                            PieceType::Queen => 'Q',
                            PieceType::King => 'K',
                        };
                        if color == Color::White { base } else { base.to_ascii_lowercase() }
                    }
                    None => '.',
                };
                s.push(c);
                s.push(' ');
            }
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_initialization() {
        let board = Board::new();
        assert_eq!(board.get_piece(0, 0), Some(Piece { piece_type: PieceType::Rook, color: Color::White }));
        assert_eq!(board.get_piece(0, 1), Some(Piece { piece_type: PieceType::Knight, color: Color::White }));
        assert_eq!(board.get_piece(7, 0), Some(Piece { piece_type: PieceType::Rook, color: Color::Black }));
        assert_eq!(board.get_piece(7, 7), Some(Piece { piece_type: PieceType::Rook, color: Color::Black }));
        assert_eq!(board.get_piece(3, 3), None);
    }

    #[test]
    fn test_set_and_get_piece() {
        let mut board = Board::new();
        let new_piece = Piece { piece_type: PieceType::Queen, color: Color::White };
        board.set_piece(4, 4, Some(new_piece));
        assert_eq!(board.get_piece(4, 4), Some(new_piece));
        board.set_piece(4, 4, None);
        assert_eq!(board.get_piece(4, 4), None);
    }

    #[test]
    fn test_display_board() {
        let board = Board::new();
        let display_string = board.display();
        assert!(display_string.len() > 0);
    }

    #[test]
    fn test_color_opposite() {
        assert_eq!(Color::White.opposite(), Color::Black);
        assert_eq!(Color::Black.opposite(), Color::White);
    }

    #[test]
    fn test_square_helpers() {
        assert_eq!(square_index(0, 0), 0);
        assert_eq!(square_index(7, 7), 63);
        assert_eq!(rank_and_file(0), (0, 0));
        assert_eq!(rank_and_file(63), (7, 7));
        assert_eq!(rank_and_file(27), (3, 3));
        assert_eq!(square_to_algebraic(0), "a1");
        assert_eq!(square_to_algebraic(63), "h8");
        assert_eq!(algebraic_to_square("e4"), Some(square_index(3, 4)));
        assert_eq!(algebraic_to_square("a1"), Some(0));
        assert_eq!(algebraic_to_square("h8"), Some(63));
    }

    #[test]
    fn test_is_attacked_starting_position() {
        let board = Board::new();
        // Center squares aren't attacked in starting position
        assert!(!board.is_square_attacked(square_index(3, 3), Color::White));
        assert!(!board.is_square_attacked(square_index(3, 3), Color::Black));
        // e2 pawn is defended by king
        assert!(board.is_square_attacked(square_index(1, 4), Color::White));
        // d2 pawn is defended by queen
        assert!(board.is_square_attacked(square_index(1, 3), Color::White));
    }

    #[test]
    fn test_castling_rights() {
        let all = CastlingRights::all();
        assert_eq!(all.to_fen_string(), "KQkq");
        let none = CastlingRights::none();
        assert_eq!(none.to_fen_string(), "-");
    }

    #[test]
    fn test_piece_type_char_roundtrip() {
        for pt in &[PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen, PieceType::King] {
            let c = pt.to_char();
            assert_eq!(PieceType::from_char(c), Some(*pt));
            assert_eq!(PieceType::from_char(c.to_ascii_lowercase()), Some(*pt));
        }
    }
}
