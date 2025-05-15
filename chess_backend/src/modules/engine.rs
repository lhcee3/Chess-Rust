use crate::board::Board;
use crate::move_gen::generate_moves;

pub struct ChessEngine {
    board: Board,
}

impl ChessEngine {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
        }
    }

    pub fn search(&mut self) -> Option<(usize, usize)> {
        let legal_moves = self.generate_all_legal_moves();

        if legal_moves.is_empty() {
            return None;
        }

        legal_moves.into_iter().next()
    }

    fn generate_all_legal_moves(&mut self) -> Vec<(usize, usize)> {
        let mut all_legal_moves = Vec::new();
         for rank in 0..8 {
            for file in 0..8{
                let moves = generate_moves(&self.board, rank, file);
                for m in moves{
                     all_legal_moves.push(m);
                }
            }
         }
         all_legal_moves
    }

    pub fn apply_move(&mut self, from: (usize, usize), to: (usize, usize)) {
        let piece = self.board.get_piece(from.0, from.1);
        self.board.set_piece(from.0, from.1, None);
        self.board.set_piece(to.0, to.1, piece);
    }

    pub fn get_board(&self) -> &Board {
        &self.board
    }

     pub fn get_board_mut(&mut self) -> &mut Board {
        &mut self.board
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_engine_initialization() {
        let engine = ChessEngine::new();
        let board = engine.get_board();
        assert_eq!(board.get_piece(0,0).unwrap().piece_type, crate::board::PieceType::Rook);
    }

    #[test]
    fn test_engine_search_no_moves() {
        let mut engine = ChessEngine::new();
        engine.get_board_mut().pieces = [None; 64];
        let best_move = engine.search();
        assert_eq!(best_move, None);
    }

    #[test]
    fn test_engine_apply_move() {
        let mut engine = ChessEngine::new();
        let initial_board = engine.get_board().clone();
        let from = (0, 0);
        let to = (0, 1);
        engine.apply_move(from, to);
        let moved_board = engine.get_board();
        assert_ne!(&initial_board, moved_board);
        assert_eq!(moved_board.get_piece(to.0, to.1), initial_board.get_piece(from.0, from.1));
        assert_eq!(moved_board.get_piece(from.0, from.1), None);
    }
}