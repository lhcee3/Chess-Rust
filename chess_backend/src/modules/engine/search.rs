use super::eval;
use super::tt::TranspositionTable;
use super::ordering::{SearchStack, score_move, update_killers, update_history, reset_history};
use super::time::TimeManager;
use crate::board::{
    Board, Color, PieceType, Piece, Move, CastlingRights, square_index, rank_and_file,
};
use crate::move_gen;

const MAX_PLY: i8 = 127;
const MATE_SCORE: i32 = 1_000_000;
const DRAW_SCORE: i32 = 0;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub depth_limit: u8,
    pub time_limit_ms: u64,
    pub use_quiescence: bool,
    pub use_transposition_table: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig { depth_limit: 8, time_limit_ms: 2000, use_quiescence: true, use_transposition_table: true }
    }
}

pub struct Position {
    pub board: Board,
    pub side_to_move: Color,
    pub castling_rights: CastlingRights,
    pub en_passant_target: Option<usize>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
}

impl Position {
    pub fn from_game(game: &crate::game::Game) -> Self {
        Position {
            board: game.board.clone(),
            side_to_move: game.side_to_move,
            castling_rights: game.castling_rights,
            en_passant_target: game.en_passant_target,
            halfmove_clock: game.halfmove_clock,
            fullmove_number: game.fullmove_number,
        }
    }

    pub fn make_move(&mut self, mv: &Move) -> UndoInfo {
        let piece = self.board.pieces[mv.from].unwrap();
        let captured = self.board.pieces[mv.to];
        let undo = UndoInfo {
            captured,
            castling_rights: self.castling_rights,
            en_passant_target: self.en_passant_target,
            halfmove_clock: self.halfmove_clock,
        };

        self.board.pieces[mv.to] = Some(piece);
        self.board.pieces[mv.from] = None;

        if mv.is_castle {
            let rank = rank_and_file(mv.from).0;
            if mv.to > mv.from {
                self.board.pieces[square_index(rank, 7)] = None;
                self.board.pieces[square_index(rank, 5)] = Some(Piece { piece_type: PieceType::Rook, color: piece.color });
            } else {
                self.board.pieces[square_index(rank, 0)] = None;
                self.board.pieces[square_index(rank, 3)] = Some(Piece { piece_type: PieceType::Rook, color: piece.color });
            }
        }

        if let Some(promotion) = mv.promotion {
            self.board.pieces[mv.to] = Some(Piece { piece_type: promotion, color: piece.color });
        }

        if piece.piece_type == PieceType::Pawn && mv.is_en_passant {
            let ep_capture = if piece.color == Color::White { mv.to - 8 } else { mv.to + 8 };
            self.board.pieces[ep_capture] = None;
        }

        // Update castling rights
        if piece.piece_type == PieceType::King {
            if piece.color == Color::White {
                self.castling_rights.white_kingside = false;
                self.castling_rights.white_queenside = false;
            } else {
                self.castling_rights.black_kingside = false;
                self.castling_rights.black_queenside = false;
            }
        }
        if mv.from == 0 || mv.to == 0 { self.castling_rights.white_queenside = false; }
        if mv.from == 7 || mv.to == 7 { self.castling_rights.white_kingside = false; }
        if mv.from == 56 || mv.to == 56 { self.castling_rights.black_queenside = false; }
        if mv.from == 63 || mv.to == 63 { self.castling_rights.black_kingside = false; }

        // Update en passant
        self.en_passant_target = if piece.piece_type == PieceType::Pawn {
            let (fr, _) = rank_and_file(mv.from);
            let (tr, tf) = rank_and_file(mv.to);
            if (tr as i8 - fr as i8).abs() == 2 {
                Some(square_index((fr + tr) / 2, tf))
            } else { None }
        } else { None };

        self.halfmove_clock = if piece.piece_type == PieceType::Pawn || captured.is_some() {
            0
        } else {
            self.halfmove_clock + 1
        };

        if piece.color == Color::Black {
            self.fullmove_number += 1;
        }
        self.side_to_move = self.side_to_move.opposite();
        undo
    }

    pub fn unmake_move(&mut self, mv: &Move, undo: &UndoInfo) {
        self.side_to_move = self.side_to_move.opposite();
        let piece = self.board.pieces[mv.to].unwrap();

        if mv.promotion.is_some() {
            self.board.pieces[mv.from] = Some(Piece { piece_type: PieceType::Pawn, color: piece.color });
        } else {
            self.board.pieces[mv.from] = Some(piece);
        }

        self.board.pieces[mv.to] = undo.captured;

        if mv.is_castle {
            let rank = rank_and_file(mv.from).0;
            if mv.to > mv.from {
                self.board.pieces[square_index(rank, 7)] = Some(Piece { piece_type: PieceType::Rook, color: piece.color });
                self.board.pieces[square_index(rank, 5)] = None;
            } else {
                self.board.pieces[square_index(rank, 0)] = Some(Piece { piece_type: PieceType::Rook, color: piece.color });
                self.board.pieces[square_index(rank, 3)] = None;
            }
        }

        if piece.piece_type == PieceType::Pawn && mv.is_en_passant {
            let ep_capture = if self.side_to_move == Color::White { mv.to - 8 } else { mv.to + 8 };
            self.board.pieces[ep_capture] = Some(Piece { piece_type: PieceType::Pawn, color: self.side_to_move.opposite() });
        }

        self.castling_rights = undo.castling_rights;
        self.en_passant_target = undo.en_passant_target;
        self.halfmove_clock = undo.halfmove_clock;
    }

    pub fn in_check(&self) -> bool {
        let king_sq = find_king(&self.board, self.side_to_move);
        self.board.is_square_attacked(king_sq, self.side_to_move.opposite())
    }

    pub fn is_draw(&self) -> bool {
        self.halfmove_clock >= 100
    }
}

pub struct UndoInfo {
    pub captured: Option<Piece>,
    pub castling_rights: CastlingRights,
    pub en_passant_target: Option<usize>,
    pub halfmove_clock: u32,
}

fn find_king(board: &Board, color: Color) -> usize {
    for i in 0..64 {
        if let Some(p) = board.pieces[i] {
            if p.piece_type == PieceType::King && p.color == color { return i; }
        }
    }
    0
}

fn generate_legal_moves(pos: &Position) -> Vec<Move> {
    let mut pseudo = Vec::new();
    for sq in 0..64 {
        if let Some(p) = pos.board.pieces[sq] {
            if p.color == pos.side_to_move {
                pseudo.extend(move_gen::generate_moves(
                    &pos.board, sq, pos.side_to_move,
                    pos.en_passant_target, pos.castling_rights,
                ));
            }
        }
    }
    pseudo.into_iter().filter(|mv| {
        let mut temp = pos.board.clone();
        apply_move_to_board(&mut temp, mv, pos.side_to_move);
        let king_sq = find_king(&temp, pos.side_to_move);
        !temp.is_square_attacked(king_sq, pos.side_to_move.opposite())
    }).collect()
}

fn apply_move_to_board(board: &mut Board, mv: &Move, color: Color) {
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
        let (_, cf) = rank_and_file(mv.to);
        board.pieces[square_index(from_rank, cf)] = None;
    }

    board.pieces[mv.from] = None;
    if let Some(promo) = mv.promotion {
        board.pieces[mv.to] = Some(Piece { piece_type: promo, color });
    } else {
        board.pieces[mv.to] = Some(Piece { piece_type: mv.piece, color });
    }
}

pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
}

pub fn best_move(game: &crate::game::Game, cfg: &EngineConfig) -> SearchResult {
    let mut pos = Position::from_game(game);
    let legal = generate_legal_moves(&pos);

    if legal.is_empty() {
        return SearchResult { best_move: None, score: if pos.in_check() { -MATE_SCORE } else { 0 }, depth: 0, nodes: 0, nps: 0, time_ms: 0 };
    }
    if legal.len() == 1 {
        return SearchResult { best_move: Some(legal[0]), score: 0, depth: 0, nodes: 0, nps: 0, time_ms: 0 };
    }

    let mut tt = TranspositionTable::new(64);
    let mut ss = SearchStack::new();
    let tm = TimeManager::new(cfg.time_limit_ms);
    let mut total_nodes = 0u64;
    let mut best = legal[0];
    let mut best_score = 0;
    let max_depth = cfg.depth_limit.min(MAX_PLY as u8);

    for depth in 1..=max_depth {
        reset_history(&mut ss);
        let mut alpha = -MATE_SCORE;
        let beta = MATE_SCORE;

        for mv in &legal {
            let undo = pos.make_move(mv);
            if !pos.in_check() {
                let score = -alpha_beta(&mut pos, depth as i8 - 1, -beta, -alpha, 1, &mut ss, &mut tt, &tm, &mut total_nodes, cfg);
                if score > alpha {
                    alpha = score;
                    best = *mv;
                    best_score = score;
                }
            }
            pos.unmake_move(mv, &undo);
        }

        if tm.soft_exceeded() { break; }
    }

    let elapsed = tm.elapsed_ms().max(1);
    SearchResult {
        best_move: Some(best),
        score: best_score,
        depth: max_depth,
        nodes: total_nodes,
        nps: (total_nodes * 1000) / elapsed,
        time_ms: elapsed,
    }
}

fn alpha_beta(
    pos: &mut Position,
    depth: i8,
    mut alpha: i32,
    beta: i32,
    ply: i8,
    ss: &mut SearchStack,
    tt: &mut TranspositionTable,
    tm: &TimeManager,
    nodes: &mut u64,
    cfg: &EngineConfig,
) -> i32 {
    if tm.hard_exceeded() { return eval::evaluate(&pos.board, pos.side_to_move); }
    *nodes += 1;
    if pos.is_draw() { return DRAW_SCORE; }
    if ply > MAX_PLY as i8 { return eval::evaluate(&pos.board, pos.side_to_move); }

    let in_check = pos.in_check();
    let pv_node = beta - alpha > 1;

    let (_tt_hit, _tt_score_opt, tt_move): (bool, Option<i32>, Option<Move>) = (false, None, None);

    if depth <= 0 {
        return if cfg.use_quiescence { quiescence(pos, alpha, beta, ply, nodes, tm, cfg) }
               else { eval::evaluate(&pos.board, pos.side_to_move) };
    }

    // Mate distance pruning
    let mating = MATE_SCORE - ply as i32;
    if mating < beta { return beta.max(mating); }
    if -mating > alpha { alpha = alpha.max(-mating); }

    // Null move pruning
    if !pv_node && !in_check && depth >= 3 && has_non_pawn_material(pos, pos.side_to_move) {
        let r = 2 + depth / 4;
        let old_stm = pos.side_to_move;
        pos.side_to_move = pos.side_to_move.opposite();
        let score = -alpha_beta(pos, depth - 1 - r, -beta, -beta + 1, ply + 1, ss, tt, tm, nodes, cfg);
        pos.side_to_move = old_stm;
        if score >= beta { return score; }
    }

    let mut moves = generate_legal_moves(pos);
    if moves.is_empty() {
        return if in_check { -(MATE_SCORE - ply as i32) } else { DRAW_SCORE };
    }

    moves.sort_by(|a, b| {
        let sa = score_move(a, tt_move, ss);
        let sb = score_move(b, tt_move, ss);
        sb.cmp(&sa)
    });

    let mut searched = 0i32;
    for mv in &moves {
        let undo = pos.make_move(mv);
        let score = if searched == 0 {
            -alpha_beta(pos, depth - 1, -beta, -alpha, ply + 1, ss, tt, tm, nodes, cfg)
        } else {
            let reduction: i8 = if searched >= 3 && !in_check && mv.captured.is_none() && !mv.is_castle {
                ((1 + (searched.min(6)) / 3) as i8).min(depth - 1)
            } else { 0 };
            if reduction > 0 {
                let s = -alpha_beta(pos, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, ss, tt, tm, nodes, cfg);
                if s > alpha {
                    -alpha_beta(pos, depth - 1, -alpha - 1, -alpha, ply + 1, ss, tt, tm, nodes, cfg)
                } else { s }
            } else {
                -alpha_beta(pos, depth - 1, -alpha - 1, -alpha, ply + 1, ss, tt, tm, nodes, cfg)
            }
        };
        pos.unmake_move(mv, &undo);
        searched += 1;

        if score > alpha {
            alpha = score;
            if !pv_node && alpha >= beta {
                if mv.captured.is_none() { update_killers(ss, mv); update_history(ss, mv, depth); }
                break;
            }
        }
    }
    alpha
}

fn quiescence(
    pos: &mut Position,
    mut alpha: i32,
    beta: i32,
    ply: i8,
    nodes: &mut u64,
    tm: &TimeManager,
    cfg: &EngineConfig,
) -> i32 {
    *nodes += 1;
    if tm.hard_exceeded() { return eval::evaluate(&pos.board, pos.side_to_move); }

    let stand_pat = eval::evaluate(&pos.board, pos.side_to_move);
    if stand_pat >= beta { return beta; }
    if alpha < stand_pat { alpha = stand_pat; }

    let moves = generate_legal_moves(pos);
    let captures: Vec<&Move> = moves.iter().filter(|m| m.captured.is_some() || m.promotion.is_some()).collect();

    for mv in captures {
        let undo = pos.make_move(mv);
        let score = -quiescence(pos, -beta, -alpha, ply + 1, nodes, tm, cfg);
        pos.unmake_move(mv, &undo);
        if score >= beta { return beta; }
        if score > alpha { alpha = score; }
    }
    alpha
}

fn has_non_pawn_material(pos: &Position, color: Color) -> bool {
    for sq in 0..64 {
        if let Some(p) = pos.board.pieces[sq] {
            if p.color == color && p.piece_type != PieceType::Pawn { return true; }
        }
    }
    false
}
