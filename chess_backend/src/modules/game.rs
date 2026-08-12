use crate::board::{
    Board, Color, PieceType, Piece, Move, CastlingRights,
    square_index, rank_and_file, square_to_algebraic,
};
use crate::move_gen;
use crate::engine::EngineConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Playing,
    Check,
    Checkmate(Color),
    Stalemate,
}

#[derive(Debug, Clone)]
pub struct Game {
    pub board: Board,
    pub side_to_move: Color,
    pub castling_rights: CastlingRights,
    pub en_passant_target: Option<usize>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
    pub move_history: Vec<Move>,
    pub status: GameStatus,
    pub ai_side: Option<Color>,
    pub engine_config: EngineConfig,
}

impl Game {
    pub fn new() -> Self {
        Game {
            board: Board::new(),
            side_to_move: Color::White,
            castling_rights: CastlingRights::all(),
            en_passant_target: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            move_history: Vec::new(),
            status: GameStatus::Playing,
            ai_side: Some(Color::Black),
            engine_config: EngineConfig::default(),
        }
    }

    pub fn get_legal_moves(&self) -> Vec<Move> {
        let color = self.side_to_move;
        let pseudo = self.get_pseudo_legal_moves();
        pseudo.into_iter().filter(|mv| {
            let mut board_copy = self.board.clone();
            apply_move_to_board(&mut board_copy, mv, color, self.en_passant_target);

            let king_square = find_king(&board_copy, color);
            !board_copy.is_square_attacked(king_square, color.opposite())
        }).collect()
    }

    fn get_pseudo_legal_moves(&self) -> Vec<Move> {
        let mut all = Vec::new();
        for sq in 0..64 {
            if let Some(p) = self.board.pieces[sq] {
                if p.color == self.side_to_move {
                    all.extend(move_gen::generate_moves(
                        &self.board, sq, self.side_to_move,
                        self.en_passant_target, self.castling_rights,
                    ));
                }
            }
        }
        all
    }

    pub fn make_move(&mut self, mv: &Move) -> Result<(), String> {
        let legal = self.get_legal_moves();
        if !legal.iter().any(|m| m.from == mv.from && m.to == mv.to
            && m.promotion == mv.promotion && m.is_castle == mv.is_castle)
        {
            return Err(format!("Illegal move: {} {}", square_to_algebraic(mv.from), square_to_algebraic(mv.to)));
        }

        let actual = legal.into_iter().find(|m| m.from == mv.from && m.to == mv.to
            && m.promotion == mv.promotion && m.is_castle == mv.is_castle).unwrap();

        self.execute_move(&actual);

        self.side_to_move = self.side_to_move.opposite();

        let opponent_moves = self.get_legal_moves();
        self.status = if opponent_moves.is_empty() {
            if self.is_check() {
                GameStatus::Checkmate(self.side_to_move)
            } else {
                GameStatus::Stalemate
            }
        } else if self.is_check() {
            GameStatus::Check
        } else {
            GameStatus::Playing
        };

        Ok(())
    }

    pub fn from_fen(&mut self, fen: &str) -> Result<(), String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err("FEN must have at least 4 fields".to_string());
        }
        self.board = Board::from_fen(parts[0]);
        self.side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err("Invalid side to move in FEN".to_string()),
        };
        let cr = parts[2];
        self.castling_rights = CastlingRights {
            white_kingside: cr.contains('K'),
            white_queenside: cr.contains('Q'),
            black_kingside: cr.contains('k'),
            black_queenside: cr.contains('q'),
        };
        self.en_passant_target = if parts[3] == "-" {
            None
        } else {
            crate::board::algebraic_to_square(parts[3])
        };
        self.halfmove_clock = parts.get(4).and_then(|p| p.parse().ok()).unwrap_or(0);
        self.fullmove_number = parts.get(5).and_then(|p| p.parse().ok()).unwrap_or(1);
        self.move_history.clear();
        self.status = GameStatus::Playing;
        Ok(())
    }

    pub fn make_move_uci(&mut self, uci: &str) -> Result<(), String> {
        let uci = uci.trim();
        if uci.len() < 4 {
            return Err("Move too short".to_string());
        }
        let from_str = &uci[0..2];
        let to_str = &uci[2..4];
        let promotion = if uci.len() > 4 {
            PieceType::from_char(uci.chars().nth(4).unwrap())
        } else {
            None
        };

        let from = crate::board::algebraic_to_square(from_str).ok_or("Invalid from square")?;
        let to = crate::board::algebraic_to_square(to_str).ok_or("Invalid to square")?;

        let piece = self.board.pieces[from].ok_or("No piece at source")?.piece_type;

        let mut mv = Move {
            from, to, piece,
            captured: self.board.pieces[to].map(|p| p.piece_type),
            promotion,
            is_castle: false,
            is_en_passant: false,
        };

        let (_from_rank, from_file) = rank_and_file(from);
        let (_to_rank, to_file) = rank_and_file(to);

        if piece == PieceType::King && (from_file as i8 - to_file as i8).abs() == 2 {
            mv.is_castle = true;
        }

        if piece == PieceType::Pawn {
            if let Some(ep) = self.en_passant_target {
                if to == ep && to_file != from_file {
                    mv.is_en_passant = true;
                    mv.captured = Some(PieceType::Pawn);
                }
            }
        }

        self.make_move(&mv)
    }

    fn execute_move(&mut self, mv: &Move) {
        let (from_rank, _) = rank_and_file(mv.from);
        let (_to_rank, _) = rank_and_file(mv.to);

        let is_capture = mv.captured.is_some();
        let is_pawn_move = mv.piece == PieceType::Pawn;

        if mv.is_castle {
            let rank = from_rank;
            if mv.to > mv.from {
                self.board.pieces[square_index(rank, 7)] = None;
                self.board.pieces[square_index(rank, 5)] = Some(Piece { piece_type: PieceType::Rook, color: self.side_to_move });
            } else {
                self.board.pieces[square_index(rank, 0)] = None;
                self.board.pieces[square_index(rank, 3)] = Some(Piece { piece_type: PieceType::Rook, color: self.side_to_move });
            }
        }

        if mv.is_en_passant {
            let captured_rank = from_rank;
            let (_, captured_file) = rank_and_file(mv.to);
            self.board.pieces[square_index(captured_rank, captured_file)] = None;
        }

        self.board.pieces[mv.from] = None;

        if let Some(promo) = mv.promotion {
            self.board.pieces[mv.to] = Some(Piece { piece_type: promo, color: self.side_to_move });
        } else {
            self.board.pieces[mv.to] = Some(Piece { piece_type: mv.piece, color: self.side_to_move });
        }

        self.update_castling_rights(mv);
        self.update_en_passant(mv);
        self.halfmove_clock = if is_capture || is_pawn_move { 0 } else { self.halfmove_clock + 1 };
        if self.side_to_move == Color::Black {
            self.fullmove_number += 1;
        }

        self.move_history.push(*mv);
    }

    fn update_castling_rights(&mut self, mv: &Move) {
        if mv.piece == PieceType::King {
            if self.side_to_move == Color::White {
                self.castling_rights.white_kingside = false;
                self.castling_rights.white_queenside = false;
            } else {
                self.castling_rights.black_kingside = false;
                self.castling_rights.black_queenside = false;
            }
        }

        if mv.from == 0 || mv.to == 0 {
            self.castling_rights.white_queenside = false;
        }
        if mv.from == 7 || mv.to == 7 {
            self.castling_rights.white_kingside = false;
        }
        if mv.from == 56 || mv.to == 56 {
            self.castling_rights.black_queenside = false;
        }
        if mv.from == 63 || mv.to == 63 {
            self.castling_rights.black_kingside = false;
        }
    }

    fn update_en_passant(&mut self, mv: &Move) {
        if mv.piece == PieceType::Pawn {
            let (from_rank, _) = rank_and_file(mv.from);
            let (to_rank, _) = rank_and_file(mv.to);
            if (to_rank as i8 - from_rank as i8).abs() == 2 {
                self.en_passant_target = Some(square_index(
                    (from_rank + to_rank) / 2,
                    rank_and_file(mv.to).1,
                ));
                return;
            }
        }
        self.en_passant_target = None;
    }

    pub fn is_check(&self) -> bool {
        let king_square = find_king(&self.board, self.side_to_move);
        self.board.is_square_attacked(king_square, self.side_to_move.opposite())
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::new();

        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let idx = square_index(rank, file);
                if let Some(p) = self.board.pieces[idx] {
                    if empty > 0 {
                        fen.push_str(&empty.to_string());
                        empty = 0;
                    }
                    let c = p.piece_type.to_char();
                    if p.color == Color::White {
                        fen.push(c);
                    } else {
                        fen.push(c.to_ascii_lowercase());
                    }
                } else {
                    empty += 1;
                }
            }
            if empty > 0 {
                fen.push_str(&empty.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');
        fen.push(if self.side_to_move == Color::White { 'w' } else { 'b' });
        fen.push(' ');
        fen.push_str(&self.castling_rights.to_fen_string());
        fen.push(' ');

        if let Some(ep) = self.en_passant_target {
            fen.push_str(&square_to_algebraic(ep));
        } else {
            fen.push('-');
        }

        fen.push(' ');
        fen.push_str(&self.halfmove_clock.to_string());
        fen.push(' ');
        fen.push_str(&self.fullmove_number.to_string());

        fen
    }

    pub fn move_to_uci(&self, mv: &Move) -> String {
        let mut s = String::with_capacity(5);
        s.push_str(&square_to_algebraic(mv.from));
        s.push_str(&square_to_algebraic(mv.to));
        if let Some(p) = mv.promotion {
            s.push(p.to_char().to_ascii_lowercase());
        }
        s
    }

    pub fn get_game_state_json(&self) -> serde_json::Value {
        let legal = self.get_legal_moves();
        let legal_ucis: Vec<String> = legal.iter().map(|m| self.move_to_uci(m)).collect();

        serde_json::json!({
            "fen": self.to_fen(),
            "sideToMove": if self.side_to_move == Color::White { "white" } else { "black" },
            "status": match self.status {
                GameStatus::Playing => "playing",
                GameStatus::Check => "check",
                GameStatus::Checkmate(_) => "checkmate",
                GameStatus::Stalemate => "stalemate",
            },
            "winner": match self.status {
                GameStatus::Checkmate(c) => Some(if c == Color::White { "white" } else { "black" }),
                _ => None,
            },
            "legalMoves": legal_ucis,
            "moveHistory": self.move_history.iter().map(|m| self.move_to_uci(m)).collect::<Vec<_>>(),
            "aiSide": self.ai_side.map(|c| if c == Color::White { "white" } else { "black" }),
            "myTurn": self.ai_side.map(|a| a != self.side_to_move).unwrap_or(true),
            "engine": {
                "depth": self.engine_config.depth_limit,
                "timeLimitMs": self.engine_config.time_limit_ms,
            },
        })
    }
}

fn find_king(board: &Board, color: Color) -> usize {
    for i in 0..64 {
        if let Some(p) = board.pieces[i] {
            if p.piece_type == PieceType::King && p.color == color {
                return i;
            }
        }
    }
    0
}

fn apply_move_to_board(board: &mut Board, mv: &Move, color: Color, _en_passant_target: Option<usize>) {
    let (from_rank, _) = rank_and_file(mv.from);

    if mv.is_castle {
        let rank = from_rank;
        if mv.to > mv.from {
            board.pieces[square_index(rank, 7)] = None;
            board.pieces[square_index(rank, 5)] = Some(Piece { piece_type: PieceType::Rook, color });
        } else {
            board.pieces[square_index(rank, 0)] = None;
            board.pieces[square_index(rank, 3)] = Some(Piece { piece_type: PieceType::Rook, color });
        }
    }

    if mv.is_en_passant {
        let (_, captured_file) = rank_and_file(mv.to);
        board.pieces[square_index(from_rank, captured_file)] = None;
    }

    board.pieces[mv.from] = None;

    if let Some(promo) = mv.promotion {
        board.pieces[mv.to] = Some(Piece { piece_type: promo, color });
    } else {
        board.pieces[mv.to] = Some(Piece { piece_type: mv.piece, color });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_initialization() {
        let game = Game::new();
        assert_eq!(game.side_to_move, Color::White);
        assert!(game.get_legal_moves().len() > 0);
        assert_eq!(game.status, GameStatus::Playing);
    }

    #[test]
    fn test_initial_fen() {
        let game = Game::new();
        let fen = game.to_fen();
        assert!(fen.starts_with("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"));
        assert!(fen.contains(" w "));
        assert!(fen.contains("KQkq"));
    }

    #[test]
    fn test_legal_moves_starting_position() {
        let game = Game::new();
        let moves = game.get_legal_moves();
        // White should have 20 legal moves in the starting position
        assert_eq!(moves.len(), 20);
    }

    #[test]
    fn test_make_move_updates_state() {
        let mut game = Game::new();
        // Move e2 pawn to e4
        let result = game.make_move_uci("e2e4");
        assert!(result.is_ok());
        assert_eq!(game.side_to_move, Color::Black);
        assert_eq!(game.move_history.len(), 1);
    }

    #[test]
    fn test_illegal_move_rejected() {
        let mut game = Game::new();
        // e2e5 is illegal (pawn can't move 3 squares)
        let result = game.make_move_uci("e2e5");
        assert!(result.is_err());
    }

    #[test]
    fn test_scholars_mate() {
        let mut game = Game::new();
        // 1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7#
        assert!(game.make_move_uci("e2e4").is_ok());
        assert!(game.make_move_uci("e7e5").is_ok());
        assert!(game.make_move_uci("f1c4").is_ok());
        assert!(game.make_move_uci("b8c6").is_ok());
        assert!(game.make_move_uci("d1h5").is_ok());
        assert!(game.make_move_uci("g8f6").is_ok());
        assert!(game.make_move_uci("h5f7").is_ok());
        assert_eq!(game.status, GameStatus::Checkmate(Color::Black));
    }

    #[test]
    fn test_en_passant() {
        let mut game = Game::new();
        // 1. e4 d5 2. e5 f5 3. exf6 (en passant)
        assert!(game.make_move_uci("e2e4").is_ok());
        assert!(game.make_move_uci("d7d5").is_ok());
        assert!(game.make_move_uci("e4e5").is_ok());
        assert!(game.make_move_uci("f7f5").is_ok());
        // e5 pawn captures f5 en passant
        assert!(game.make_move_uci("e5f6").is_ok());
        assert_eq!(game.board.pieces[21], None); // f5 pawn removed
    }

    #[test]
    fn test_castling() {
        let mut game = Game::new();
        // Clear pieces for castling: 1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5
        assert!(game.make_move_uci("e2e4").is_ok());
        assert!(game.make_move_uci("e7e5").is_ok());
        assert!(game.make_move_uci("g1f3").is_ok());
        assert!(game.make_move_uci("b8c6").is_ok());
        assert!(game.make_move_uci("f1c4").is_ok());
        assert!(game.make_move_uci("f8c5").is_ok());
        // 4. O-O (kingside castle)
        assert!(game.make_move_uci("e1g1").is_ok());
        // Verify king and rook moved
        assert_eq!(game.board.pieces[6].map(|p| p.piece_type), Some(PieceType::King));
        assert_eq!(game.board.pieces[5].map(|p| p.piece_type), Some(PieceType::Rook));
    }
}
