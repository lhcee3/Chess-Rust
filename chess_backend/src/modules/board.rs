#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub piece_type: PieceType,
    pub color: Color,
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
        pieces[8] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[9] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[10] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[11] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[12] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[13] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[14] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });
        pieces[15] = Some(Piece { piece_type: PieceType::Pawn, color: Color::White });

        pieces[48] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[49] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[50] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[51] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[52] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[53] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[54] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[55] = Some(Piece { piece_type: PieceType::Pawn, color: Color::Black });
        pieces[56] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });
        pieces[57] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        pieces[58] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        pieces[59] = Some(Piece { piece_type: PieceType::Queen, color: Color::Black });
        pieces[60] = Some(Piece { piece_type: PieceType::King, color: Color::Black });
        pieces[61] = Some(Piece { piece_type: PieceType::Bishop, color: Color::Black });
        pieces[62] = Some(Piece { piece_type: PieceType::Knight, color: Color::Black });
        pieces[63] = Some(Piece { piece_type: PieceType::Rook, color: Color::Black });

        Self { pieces }
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

    pub fn display(&self) -> String {
        let mut s = String::new();
        for rank in (0..8).rev() {
            for file in 0..8 {
                let piece = self.get_piece(rank, file);
                let c = match piece {
                    Some(Piece { piece_type: PieceType::Pawn, color: Color::White }) => 'P',
                    Some(Piece { piece_type: PieceType::Knight, color: Color::White }) => 'N',
                    Some(Piece { piece_type: PieceType::Bishop, color: Color::White }) => 'B',
                    Some(Piece { piece_type: PieceType::Rook, color: Color::White }) => 'R',
                    Some(Piece { piece_type: PieceType::Queen, color: Color::White }) => 'Q',
                    Some(Piece { piece_type: PieceType::King, color: Color::White }) => 'K',
                    Some(Piece { piece_type: PieceType::Pawn, color: Color::Black }) => 'p',
                    Some(Piece { piece_type: PieceType::Knight, color: Color::Black }) => 'n',
                    Some(Piece { piece_type: PieceType::Bishop, color: Color::Black }) => 'b',
                    Some(Piece { piece_type: PieceType::Rook, color: Color::Black }) => 'r',
                    Some(Piece { piece_type: PieceType::Queen, color: Color::Black }) => 'q',
                    Some(Piece { piece_type: PieceType::King, color: Color::Black }) => 'k',
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
        println!("{}", display_string);
        assert!(display_string.len() > 0);
    }
}