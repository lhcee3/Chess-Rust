use crate::board::{
    Board, Color, PieceType, Piece, Move, CastlingRights,
    square_index, rank_and_file,
};
use crate::move_gen;
use crate::game::Game;

pub struct ChessEngine;

impl ChessEngine {
    pub fn generate_legal_moves(board: &Board, color: Color) -> Vec<Move> {
        let game = Game::new();
        let mut all = Vec::new();
        for sq in 0..64 {
            if let Some(p) = board.pieces[sq] {
                if p.color == color {
                    all.extend(move_gen::generate_moves(
                        board, sq, color, None, CastlingRights::all(),
                    ));
                }
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_engine_generates_moves() {
        let board = Board::new();
        let moves = ChessEngine::generate_legal_moves(&board, Color::White);
        assert!(!moves.is_empty());
    }
}
