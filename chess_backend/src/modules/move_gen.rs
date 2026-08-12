use crate::board::{
    Board, Color, PieceType, Piece, Move, CastlingRights,
    square_index, rank_and_file,
};

pub fn generate_moves(
    board: &Board,
    square: usize,
    color: Color,
    en_passant_target: Option<usize>,
    castling_rights: CastlingRights,
) -> Vec<Move> {
    let mut moves = Vec::new();
    let Some(piece) = board.pieces[square] else { return moves; };
    if piece.color != color { return moves; }

    let (rank, file) = rank_and_file(square);

    match piece.piece_type {
        PieceType::Pawn => generate_pawn_moves(board, square, rank, file, color, en_passant_target, &mut moves),
        PieceType::Knight => generate_knight_moves(board, square, rank, file, color, &mut moves),
        PieceType::Bishop => generate_bishop_moves(board, square, rank, file, color, &mut moves),
        PieceType::Rook => generate_rook_moves(board, square, rank, file, color, &mut moves),
        PieceType::Queen => generate_queen_moves(board, square, rank, file, color, &mut moves),
        PieceType::King => {
            generate_king_moves(board, square, rank, file, color, &mut moves);
            generate_castle_moves(board, square, color, castling_rights, &mut moves);
        }
    }

    moves
}

fn add_move(
    moves: &mut Vec<Move>,
    from: usize, to: usize,
    piece: PieceType,
    captured: Option<PieceType>,
    promotion: Option<PieceType>,
    is_castle: bool,
    is_en_passant: bool,
) {
    moves.push(Move { from, to, piece, captured, promotion, is_castle, is_en_passant });
}

fn generate_pawn_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    en_passant_target: Option<usize>,
    moves: &mut Vec<Move>,
) {
    let direction: i8 = match color {
        Color::White => 1,
        Color::Black => -1,
    };
    let new_rank = (rank as i8 + direction) as usize;
    let piece = PieceType::Pawn;

    if new_rank < 8 {
        let to = square_index(new_rank, file);
        if board.pieces[to].is_none() {
            if new_rank == 0 || new_rank == 7 {
                for &promo in &[PieceType::Queen, PieceType::Rook, PieceType::Bishop, PieceType::Knight] {
                    add_move(moves, from, to, piece, None, Some(promo), false, false);
                }
            } else {
                add_move(moves, from, to, piece, None, None, false, false);
            }

            if (rank == 1 && color == Color::White) || (rank == 6 && color == Color::Black) {
                let double_rank = (rank as i8 + 2 * direction) as usize;
                let double_to = square_index(double_rank, file);
                if board.pieces[double_to].is_none() {
                    add_move(moves, from, double_to, piece, None, None, false, false);
                }
            }
        }
    }

    for &df in &[-1i8, 1i8] {
        let cf = file as i8 + df;
        if cf >= 0 && cf < 8 && new_rank < 8 {
            let to = square_index(new_rank, cf as usize);
            if let Some(captured_piece) = board.pieces[to] {
                if captured_piece.color != color {
                    if new_rank == 0 || new_rank == 7 {
                        for &promo in &[PieceType::Queen, PieceType::Rook, PieceType::Bishop, PieceType::Knight] {
                            add_move(moves, from, to, piece, Some(captured_piece.piece_type), Some(promo), false, false);
                        }
                    } else {
                        add_move(moves, from, to, piece, Some(captured_piece.piece_type), None, false, false);
                    }
                }
            }
        }
    }

    if let Some(ep_target) = en_passant_target {
        let (ep_rank, ep_file) = rank_and_file(ep_target);
        if (ep_file as i8 - file as i8).abs() == 1 && ep_rank == new_rank {
            let captured_square = square_index(rank, ep_file);
            if let Some(p) = board.pieces[captured_square] {
                if p.color != color && p.piece_type == PieceType::Pawn {
                    add_move(moves, from, ep_target, piece, Some(PieceType::Pawn), None, false, true);
                }
            }
        }
    }
}

fn generate_knight_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    moves: &mut Vec<Move>,
) {
    let offsets: [(i8, i8); 8] = [
        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
        (1, -2), (1, 2), (2, -1), (2, 1),
    ];
    let piece = PieceType::Knight;

    for &(dr, df) in &offsets {
        let nr = rank as i8 + dr;
        let nf = file as i8 + df;
        if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
            let to = square_index(nr as usize, nf as usize);
            if let Some(p) = board.pieces[to] {
                if p.color != color {
                    add_move(moves, from, to, piece, Some(p.piece_type), None, false, false);
                }
            } else {
                add_move(moves, from, to, piece, None, None, false, false);
            }
        }
    }
}

fn generate_bishop_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    moves: &mut Vec<Move>,
) {
    let dirs: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    generate_sliding_moves(board, from, rank, file, color, PieceType::Bishop, &dirs, moves);
}

fn generate_rook_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    moves: &mut Vec<Move>,
) {
    let dirs: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    generate_sliding_moves(board, from, rank, file, color, PieceType::Rook, &dirs, moves);
}

fn generate_queen_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    moves: &mut Vec<Move>,
) {
    let dirs: [(i8, i8); 8] = [
        (1, 0), (-1, 0), (0, 1), (0, -1),
        (1, 1), (1, -1), (-1, 1), (-1, -1),
    ];
    generate_sliding_moves(board, from, rank, file, color, PieceType::Queen, &dirs, moves);
}

fn generate_king_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color,
    moves: &mut Vec<Move>,
) {
    let piece = PieceType::King;

    for dr in -1i8..=1 {
        for df in -1i8..=1 {
            if dr == 0 && df == 0 { continue; }
            let nr = rank as i8 + dr;
            let nf = file as i8 + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let to = square_index(nr as usize, nf as usize);
                if let Some(p) = board.pieces[to] {
                    if p.color != color {
                        add_move(moves, from, to, piece, Some(p.piece_type), None, false, false);
                    }
                } else {
                    add_move(moves, from, to, piece, None, None, false, false);
                }
            }
        }
    }
}

fn generate_sliding_moves(
    board: &Board, from: usize, rank: usize, file: usize, color: Color, piece: PieceType,
    directions: &[(i8, i8)], moves: &mut Vec<Move>,
) {
    for &(dr, df) in directions {
        let mut cr = rank as i8 + dr;
        let mut cf = file as i8 + df;
        while cr >= 0 && cr < 8 && cf >= 0 && cf < 8 {
            let to = square_index(cr as usize, cf as usize);
            if let Some(p) = board.pieces[to] {
                if p.color != color {
                    add_move(moves, from, to, piece, Some(p.piece_type), None, false, false);
                }
                break;
            } else {
                add_move(moves, from, to, piece, None, None, false, false);
            }
            cr += dr;
            cf += df;
        }
    }
}

fn generate_castle_moves(
    board: &Board, from: usize, color: Color, rights: CastlingRights,
    moves: &mut Vec<Move>,
) {
    let rank = if color == Color::White { 0 } else { 7 };

    let enemy = color.opposite();
    let king_square = from;

    if color == Color::White && rights.white_kingside {
        if board.pieces[square_index(rank, 5)].is_none()
            && board.pieces[square_index(rank, 6)].is_none()
            && board.pieces[square_index(rank, 7)] == Some(Piece { piece_type: PieceType::Rook, color: Color::White })
            && !board.is_square_attacked(king_square, enemy)
            && !board.is_square_attacked(square_index(rank, 5), enemy)
            && !board.is_square_attacked(square_index(rank, 6), enemy)
        {
            add_move(moves, from, square_index(rank, 6), PieceType::King, None, None, true, false);
        }
    }

    if color == Color::White && rights.white_queenside {
        if board.pieces[square_index(rank, 3)].is_none()
            && board.pieces[square_index(rank, 2)].is_none()
            && board.pieces[square_index(rank, 1)].is_none()
            && board.pieces[square_index(rank, 0)] == Some(Piece { piece_type: PieceType::Rook, color: Color::White })
            && !board.is_square_attacked(king_square, enemy)
            && !board.is_square_attacked(square_index(rank, 3), enemy)
            && !board.is_square_attacked(square_index(rank, 2), enemy)
        {
            add_move(moves, from, square_index(rank, 2), PieceType::King, None, None, true, false);
        }
    }

    if color == Color::Black && rights.black_kingside {
        if board.pieces[square_index(rank, 5)].is_none()
            && board.pieces[square_index(rank, 6)].is_none()
            && board.pieces[square_index(rank, 7)] == Some(Piece { piece_type: PieceType::Rook, color: Color::Black })
            && !board.is_square_attacked(king_square, enemy)
            && !board.is_square_attacked(square_index(rank, 5), enemy)
            && !board.is_square_attacked(square_index(rank, 6), enemy)
        {
            add_move(moves, from, square_index(rank, 6), PieceType::King, None, None, true, false);
        }
    }

    if color == Color::Black && rights.black_queenside {
        if board.pieces[square_index(rank, 3)].is_none()
            && board.pieces[square_index(rank, 2)].is_none()
            && board.pieces[square_index(rank, 1)].is_none()
            && board.pieces[square_index(rank, 0)] == Some(Piece { piece_type: PieceType::Rook, color: Color::Black })
            && !board.is_square_attacked(king_square, enemy)
            && !board.is_square_attacked(square_index(rank, 3), enemy)
            && !board.is_square_attacked(square_index(rank, 2), enemy)
        {
            add_move(moves, from, square_index(rank, 2), PieceType::King, None, None, true, false);
        }
    }
}
