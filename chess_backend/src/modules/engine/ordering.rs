use crate::board::{Move, PieceType};

const CAPTURE_BASE: i32 = 10_000;
const KILLER_BASE: i32 = 5000;
const CASTLE_BONUS: i32 = 100;
const HISTORY_MAX: i32 = 1_000_000;

const MVV_LVA: [[i32; 6]; 6] = [
    // Victim: P  N  B  R  Q  K   Attacker ↓
    [100, 200, 300, 400, 500, 600],  // Pawn captures
    [101, 202, 303, 404, 505, 606],  // Knight
    [102, 204, 306, 408, 510, 612],  // Bishop
    [103, 206, 309, 412, 515, 618],  // Rook
    [104, 208, 312, 416, 520, 624],  // Queen
    [105, 210, 315, 420, 525, 630],  // King
];

pub struct SearchStack {
    pub killers: [[Option<Move>; 2]; 128],
    pub history: [[i32; 64]; 64],
    pub ply: u8,
}

impl SearchStack {
    pub fn new() -> Self {
        SearchStack {
            killers: [[None, None]; 128],
            history: [[0; 64]; 64],
            ply: 0,
        }
    }
}

pub fn score_move(mv: &Move, tt_move: Option<Move>, ss: &SearchStack) -> i32 {
    if Some(*mv) == tt_move {
        return i32::MAX / 2;
    }

    if let Some(captured) = mv.captured {
        let attacker = mv.promotion.unwrap_or(PieceType::Pawn);
        let victim = captured;
        let mvv_lva = MVV_LVA[pt_index(victim)][pt_index(attacker)];
        return CAPTURE_BASE + mvv_lva;
    }

    if mv.is_castle {
        return CASTLE_BONUS;
    }

    let ply = ss.ply.min(127) as usize;
    if ss.killers[ply][0] == Some(*mv) {
        return KILLER_BASE;
    }
    if ss.killers[ply][1] == Some(*mv) {
        return KILLER_BASE - 1;
    }

    ss.history[mv.from][mv.to]
}

pub fn update_killers(ss: &mut SearchStack, mv: &Move) {
    let ply = ss.ply.min(127) as usize;
    if ss.killers[ply][0] != Some(*mv) {
        ss.killers[ply][1] = ss.killers[ply][0];
        ss.killers[ply][0] = Some(*mv);
    }
}

pub fn update_history(ss: &mut SearchStack, mv: &Move, depth: i8) {
    let bonus = (depth as i32) * (depth as i32);
    let from = mv.from;
    let to = mv.to;
    ss.history[from][to] = (ss.history[from][to] + bonus).min(HISTORY_MAX);
}

pub fn reset_history(ss: &mut SearchStack) {
    ss.history = [[0; 64]; 64];
    ss.killers = [[None, None]; 128];
}

fn pt_index(pt: PieceType) -> usize {
    match pt {
        PieceType::Pawn => 0,
        PieceType::Knight => 1,
        PieceType::Bishop => 2,
        PieceType::Rook => 3,
        PieceType::Queen => 4,
        PieceType::King => 5,
    }
}
