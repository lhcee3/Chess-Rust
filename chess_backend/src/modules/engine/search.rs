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

fn game_move_uci(mv: &Move) -> String {
    crate::game::Game::new().move_to_uci(mv)
}

static CFG_TEST_PRINT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn cfg_test_print() -> bool {
    CFG_TEST_PRINT.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub move_depth: u8,
    pub move_time_limit_ms: u64,
    pub analysis_depth: u8,
    pub analysis_time_limit_ms: u64,
    pub use_quiescence: bool,
    pub use_transposition_table: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            move_depth: 8,
            move_time_limit_ms: 2000,
            analysis_depth: 5,
            analysis_time_limit_ms: 5000,
            use_quiescence: true,
            use_transposition_table: true,
        }
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
    let tm = TimeManager::new(cfg.move_time_limit_ms);
    let mut total_nodes = 0u64;
    let mut best = legal[0];
    let mut best_score = 0;
    let max_depth = cfg.move_depth.min(MAX_PLY as u8);

    for depth in 1..=max_depth {
        reset_history(&mut ss);
        let mut alpha = -MATE_SCORE;
        let beta = MATE_SCORE;

        for mv in &legal {
            let undo = pos.make_move(mv);
            if !pos.in_check() {
                let score = -alpha_beta(&mut pos, depth as i8 - 1, -beta, -alpha, 1, &mut ss, &mut tt, &tm, &mut total_nodes, cfg);
                if cfg_test_print() && depth == cfg.move_depth {
                    println!("  {} -> {}", game_move_uci(mv), score);
                }
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

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub played_uci: String,
    pub classification: String,
    pub played_score_cp: i32,
    pub best_score_cp: i32,
    pub loss_cp: i32,
    pub best_move: Option<String>,
    pub alternatives: Vec<String>,
    pub reason_codes: Vec<String>,
    pub engine_depth: u8,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
}

pub fn analyze(game: &crate::game::Game, played_uci: &str, cfg: &EngineConfig) -> Option<AnalysisResult> {
    let mut pos = Position::from_game(game);
    let legal = generate_legal_moves(&pos);
    if legal.is_empty() {
        return None;
    }
    let played = legal.iter().copied().find(|m| game.move_to_uci(m) == played_uci)?;

    let mut tt = TranspositionTable::new(32);
    let mut ss = SearchStack::new();
    let tm = TimeManager::new(cfg.analysis_time_limit_ms);
    let mut nodes = 0u64;
    let max_depth = cfg.analysis_depth.min(MAX_PLY as u8).max(1);

    // Full-window score per root move so each played/alternative score is exact.
    let mut scores: Vec<(Move, i32)> = legal.iter().map(|m| (*m, 0)).collect();
    let mut reached_depth = 0u8;

    for depth in 1..=max_depth {
        reset_history(&mut ss);
        let mut completed = true;
        for entry in scores.iter_mut() {
            if tm.hard_exceeded() { completed = false; break; }
            let mv = entry.0;
            let undo = pos.make_move(&mv);
            let score = -alpha_beta(&mut pos, depth as i8 - 1, -MATE_SCORE, MATE_SCORE, 1, &mut ss, &mut tt, &tm, &mut nodes, cfg);
            pos.unmake_move(&mv, &undo);
            entry.1 = score;
        }
        reached_depth = depth;
        if !completed || tm.soft_exceeded() {
            break;
        }
    }

    let mut best = scores[0];
    for &(m, s) in &scores {
        if s > best.1 { best = (m, s); }
    }
    let played_score = scores.iter().find(|(m, _)| *m == played).map(|(_, s)| *s).unwrap_or(0);
    let best_score = best.1;
    let loss = (best_score - played_score).max(0);

    let classification = match loss {
        0 => "best",
        l if l <= 25 => "good",
        l if l <= 75 => "inaccuracy",
        l if l <= 200 => "mistake",
        _ => "blunder",
    }.to_string();

    let mut ordered = scores.clone();
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    let alternatives: Vec<String> = ordered.iter()
        .filter(|(m, _)| *m != best.0)
        .take(3)
        .map(|(m, _)| game.move_to_uci(m))
        .collect();

    // Heuristic reason codes describing the played move.
    let mut reasons: Vec<String> = Vec::new();
    if played.captured.is_some() { reasons.push("capture".to_string()); }
    if played.promotion.is_some() { reasons.push("promotion".to_string()); }
    if played.is_castle { reasons.push("king_safety".to_string()); }
    let gives_check = {
        let undo = pos.make_move(&played);
        let c = pos.in_check();
        pos.unmake_move(&played, &undo);
        c
    };
    if gives_check { reasons.push("check".to_string()); }
    let (from_rank, _) = rank_and_file(played.from);
    let (to_rank, to_file) = rank_and_file(played.to);
    if played.piece == PieceType::Pawn && (3..=4).contains(&to_rank) && (3..=4).contains(&to_file) {
        reasons.push("center_control".to_string());
    }
    if matches!(played.piece, PieceType::Knight | PieceType::Bishop) && (from_rank == 0 || from_rank == 7) {
        reasons.push("development".to_string());
    }
    if loss > 75 { reasons.push("loses_material".to_string()); }
    if reasons.is_empty() { reasons.push("quiet_move".to_string()); }

    let elapsed = tm.elapsed_ms().max(1);
    Some(AnalysisResult {
        played_uci: game.move_to_uci(&played),
        classification,
        played_score_cp: played_score,
        best_score_cp: best_score,
        loss_cp: loss,
        best_move: Some(game.move_to_uci(&best.0)),
        alternatives,
        reason_codes: reasons,
        engine_depth: reached_depth,
        nodes,
        nps: (nodes * 1000) / elapsed,
        time_ms: elapsed,
    })
}

fn alpha_beta(
    pos: &mut Position,
    depth: i8,
    mut alpha: i32,
    mut beta: i32,
    ply: i8,
    ss: &mut SearchStack,
    tt: &mut TranspositionTable,
    tm: &TimeManager,
    nodes: &mut u64,
    cfg: &EngineConfig,
) -> i32 {
    if tm.hard_exceeded() { let r = eval::evaluate(&pos.board, pos.side_to_move); if cfg_test_print() { println!("    AB-hard: ply={} r={}", ply, r); } return r; }
    *nodes += 1;
    if pos.is_draw() { if cfg_test_print() { println!("    AB-draw: ply={}", ply); } return DRAW_SCORE; }
    if ply > MAX_PLY as i8 { let r = eval::evaluate(&pos.board, pos.side_to_move); if cfg_test_print() { println!("    AB-maxply: ply={} r={}", ply, r); } return r; }

    let in_check = pos.in_check();
    let pv_node = beta - alpha > 1;

    let (_tt_hit, _tt_score_opt, tt_move): (bool, Option<i32>, Option<Move>) = (false, None, None);

    if depth <= 0 {
        let r = if cfg.use_quiescence { quiescence(pos, alpha, beta, ply, nodes, tm, cfg) }
                else { eval::evaluate(&pos.board, pos.side_to_move) };
        if cfg_test_print() { println!("    AB-depth0: ply={} r={}", ply, r); }
        return r;
    }

    // Mate distance pruning
    let mating = MATE_SCORE - ply as i32;
    if mating < beta {
        beta = mating;
        if alpha >= beta {
            if cfg_test_print() { println!("    MDP1: ply={} alpha={}", ply, alpha); }
            return alpha;
        }
    }
    let mated = -MATE_SCORE + ply as i32;
    if mated > alpha {
        alpha = mated;
        if alpha >= beta {
            if cfg_test_print() { println!("    MDP2: ply={} alpha={}", ply, alpha); }
            return alpha;
        }
    }

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
        let r = if in_check { -(MATE_SCORE - ply as i32) } else { DRAW_SCORE };
        if cfg_test_print() { println!("    EMPTY: ply={} in_check={} ret={}", ply, in_check, r); }
        return r;
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
    if cfg_test_print() { println!("    AB-end: ply={} depth={} alpha={}", ply, depth, alpha); }
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

    let in_check = pos.in_check();
    let stand_pat = eval::evaluate(&pos.board, pos.side_to_move);
    if !in_check {
        if stand_pat >= beta { return beta; }
        if alpha < stand_pat { alpha = stand_pat; }
    }

    let moves = generate_legal_moves(pos);
    if moves.is_empty() {
        return if in_check { -(MATE_SCORE - ply as i32) } else { stand_pat };
    }
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

fn perft(pos: &mut Position, depth: i8) -> u64 {
    let moves = generate_legal_moves(pos);
    if depth <= 1 {
        return moves.len() as u64;
    }
    let mut count = 0;
    for mv in &moves {
        let undo = pos.make_move(mv);
        count += perft(pos, depth - 1);
        pos.unmake_move(mv, &undo);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    fn start_pos() -> Position {
        Position::from_game(&Game::new())
    }

    #[test]
    fn perft_starting_position() {
        let mut pos = start_pos();
        assert_eq!(perft(&mut pos, 1), 20);
        assert_eq!(perft(&mut pos, 2), 400);
        assert_eq!(perft(&mut pos, 3), 8_902);
    }

    #[test]
    fn perft_kewrick_position() {
        let mut game = Game::new();
        game.from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        let mut pos = Position::from_game(&game);
        assert_eq!(perft(&mut pos, 1), 48);
        assert_eq!(perft(&mut pos, 2), 2_039);
    }

    #[test]
    fn perft_after_e4_e5() {
        let mut game = Game::new();
        game.make_move_uci("e2e4").unwrap();
        game.make_move_uci("e7e5").unwrap();
        let mut pos = Position::from_game(&game);
        assert_eq!(perft(&mut pos, 1), 29);
        assert_eq!(perft(&mut pos, 2), 835);
        assert_eq!(perft(&mut pos, 3), 24_825);
    }

    #[test]
    fn search_explores_more_than_one_thousand_nodes_at_depth_four() {
        let cfg = EngineConfig { move_depth: 4, move_time_limit_ms: 5000, analysis_depth: 5, analysis_time_limit_ms: 5000, use_quiescence: true, use_transposition_table: true };
        let result = best_move(&Game::new(), &cfg);
        assert!(result.best_move.is_some());
        assert!(result.nodes > 1_000, "depth-4 search from start should explore >1000 nodes, got {}", result.nodes);
    }

    #[test]
    fn search_finds_mate_in_one() {
        let mut game = Game::new();
        game.from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 4");
        CFG_TEST_PRINT.store(true, std::sync::atomic::Ordering::Relaxed);
        {
            let mut p = Position::from_game(&game);
            println!("eval root (white stm): {}", eval::evaluate(&p.board, p.side_to_move));
            for mv in generate_legal_moves(&p) {
                let uci = game_move_uci(&mv);
                if uci == "h5f7" || uci == "h5f5" {
                    let undo = p.make_move(&mv);
                    println!("after {}: side={:?} in_check={} eval={} legal_moves={}",
                        uci, p.side_to_move, p.in_check(),
                        eval::evaluate(&p.board, p.side_to_move),
                        generate_legal_moves(&p).len());
                    p.unmake_move(&mv, &undo);
                }
            }
        }
        let cfg = EngineConfig { move_depth: 2, move_time_limit_ms: 5000, analysis_depth: 5, analysis_time_limit_ms: 5000, use_quiescence: true, use_transposition_table: true };
        let result = best_move(&game, &cfg);
        assert!(result.best_move.is_some());
        let uci = game.move_to_uci(&result.best_move.unwrap());
        assert_eq!(uci, "h5f7", "engine must deliver Qxf7#, got {}", uci);
    }
}
