use crate::board::{Board, Color, PieceType};

const PAWN: usize = 0;
const KNIGHT: usize = 1;
const BISHOP: usize = 2;
const ROOK: usize = 3;
const QUEEN: usize = 4;
const KING: usize = 5;

const MG_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];
const EG_VALUE: [i32; 6] = [120, 300, 320, 520, 950, 0];

fn pst_value(pst: &[i32; 64], sq: usize, color: Color) -> i32 {
    let idx = if color == Color::White { sq } else { sq ^ 56 };
    pst[idx]
}

// PST from white's perspective: row 0 = rank 1 (a1), row 7 = rank 8 (a8)
const PST_MG: [[i32; 64]; 6] = [
    // Pawn
    [
         0,  0,  0,  0,  0,  0,  0,  0,
         5, 10, 10,-20,-20, 10, 10,  5,
         5,  5, 10, 25, 25, 10,  5,  5,
         0,  0,  0, 20, 20,  0,  0,  0,
         5,  5, 10, 25, 25, 10,  5,  5,
        10, 10, 20, 30, 30, 20, 10, 10,
        50, 50, 50, 50, 50, 50, 50, 50,
         0,  0,  0,  0,  0,  0,  0,  0,
    ],
    // Knight
    [
        -50,-40,-30,-30,-30,-30,-40,-50,
        -40,-20,  0,  0,  0,  0,-20,-40,
        -30,  0, 10, 15, 15, 10,  0,-30,
        -30,  5, 15, 20, 20, 15,  5,-30,
        -30,  0, 15, 20, 20, 15,  0,-30,
        -30,  5, 10, 15, 15, 10,  5,-30,
        -40,-20,  0,  5,  5,  0,-20,-40,
        -50,-40,-30,-30,-30,-30,-40,-50,
    ],
    // Bishop
    [
        -20,-10,-10,-10,-10,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0, 10, 10, 10, 10,  0,-10,
        -10,  5,  5, 10, 10,  5,  5,-10,
        -10,  0,  5, 10, 10,  5,  0,-10,
        -10, 10, 10, 10, 10, 10, 10,-10,
        -10,  5,  0,  0,  0,  0,  5,-10,
        -20,-10,-10,-10,-10,-10,-10,-20,
    ],
    // Rook
    [
         0,  0,  0,  0,  0,  0,  0,  0,
         5, 10, 10, 10, 10, 10, 10,  5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
        -5,  0,  0,  0,  0,  0,  0, -5,
         0,  0,  0,  5,  5,  0,  0,  0,
    ],
    // Queen
    [
        -20,-10,-10, -5, -5,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  5,  5,  5,  5,  5,  0,-10,
         -5,  0,  5,  5,  5,  5,  0, -5,
          0,  0,  5,  5,  5,  5,  0, -5,
        -10,  5,  5,  5,  5,  5,  0,-10,
        -10,  0,  5,  0,  0,  0,  0,-10,
        -20,-10,-10, -5, -5,-10,-10,-20,
    ],
    // King (MG)
    [
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -30,-40,-40,-50,-50,-40,-40,-30,
        -20,-30,-30,-40,-40,-30,-30,-20,
        -10,-20,-20,-20,-20,-20,-20,-10,
         20, 20,  0,  0,  0,  0, 20, 20,
         20, 30, 10,  0,  0, 10, 30, 20,
    ],
];

const PST_EG: [[i32; 64]; 6] = [
    PST_MG[0], // Pawn EG
    PST_MG[1], // Knight EG
    PST_MG[2], // Bishop EG
    PST_MG[3], // Rook EG
    PST_MG[4], // Queen EG
    // King (EG) - center in endgame
    [
        -50,-40,-30,-20,-20,-30,-40,-50,
        -30,-20,-10,  0,  0,-10,-20,-30,
        -30,-10, 20, 30, 30, 20,-10,-30,
        -30,-10, 30, 40, 40, 30,-10,-30,
        -30,-10, 30, 40, 40, 30,-10,-30,
        -30,-10, 20, 30, 30, 20,-10,-30,
        -30,-30,  0,  0,  0,  0,-30,-30,
        -50,-30,-30,-30,-30,-30,-30,-50,
    ],
];

fn pt_index(pt: PieceType) -> usize {
    match pt {
        PieceType::Pawn => PAWN,
        PieceType::Knight => KNIGHT,
        PieceType::Bishop => BISHOP,
        PieceType::Rook => ROOK,
        PieceType::Queen => QUEEN,
        PieceType::King => KING,
    }
}

pub fn evaluate(board: &Board, stm: Color) -> i32 {
    let mut white_mg = 0i32;
    let mut white_eg = 0i32;
    let mut phase = 0i32;

    for sq in 0..64 {
        if let Some(piece) = board.pieces[sq] {
            let idx = pt_index(piece.piece_type);
            let val_mg = MG_VALUE[idx];
            let val_eg = EG_VALUE[idx];
            let pst_mg = pst_value(&PST_MG[idx], sq, Color::White);
            let pst_eg = pst_value(&PST_EG[idx], sq, Color::White);

            match piece.piece_type {
                PieceType::Knight | PieceType::Bishop => phase += 4,
                PieceType::Rook => phase += 2,
                PieceType::Queen => phase += 1,
                _ => {}
            }

            if piece.color == Color::White {
                white_mg += val_mg + pst_mg;
                white_eg += val_eg + pst_eg;
            } else {
                white_mg -= val_mg + pst_mg;
                white_eg -= val_eg + pst_eg;
            }
        }
    }

    let phase = phase.min(24);
    let white_score = (white_mg * (24 - phase) + white_eg * phase) / 24;

    let score = if stm == Color::White { white_score } else { -white_score };
    score + 10 // tempo
}
