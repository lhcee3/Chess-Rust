use crate::board::{Board, Color, PieceType};

pub fn generate_moves(board: &Board, rank: usize, file: usize) -> Vec<(usize, usize)> {
    let mut moves = Vec::new();

    if rank > 7 || file > 7 {
        return moves;
    }

    let piece_option = board.get_piece(rank, file);
    if piece_option.is_none() {
        return moves;
    }
    let piece = piece_option.unwrap();

    match piece.piece_type {
        PieceType::Pawn => {
            generate_pawn_moves(board, rank, file, piece.color, &mut moves);
        }
        PieceType::Rook => {
            generate_rook_moves(board, rank, file, piece.color, &mut moves);
        }
        PieceType::Knight => {
            generate_knight_moves(board, rank, file, piece.color, &mut moves);
        }
        PieceType::Bishop => {
            generate_bishop_moves(board, rank, file, piece.color, &mut moves);
        }
        PieceType::Queen => {
            generate_queen_moves(board, rank, file, piece.color, &mut moves);
        }
        PieceType::King => {
            generate_king_moves(board, rank, file, piece.color, &mut moves);
        }
    }

    moves
}

fn generate_pawn_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    let direction: i8 = match color {
        Color::White => 1,
        Color::Black => -1,
    };
    let new_rank = (rank as i8 + direction) as usize;

    if new_rank < 8 && board.get_piece(new_rank, file).is_none() {
        moves.push((new_rank, file));

        if (rank == 1 && color == Color::White) || (rank == 6 && color == Color::Black) {
            let double_new_rank = (rank as i8 + 2 * direction) as usize;
            if board.get_piece(double_new_rank, file).is_none() {
                moves.push((double_new_rank, file));
            }
        }
    }
    let capture_files: [i8; 2] = [file as i8 - 1, file as i8 + 1];
    for &capture_file in capture_files.iter() {
        if capture_file >= 0 && capture_file < 8 {
            let capture_rank = (rank as i8 + direction) as usize;
            if capture_rank < 8 {
                if let Some(captured_piece) = board.get_piece(capture_rank, capture_file as usize) {
                    if captured_piece.color != color {
                        moves.push((capture_rank, capture_file as usize));
                    }
                }
            }
        }
    }
}

fn generate_rook_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    generate_straight_moves(board, rank, file, color, moves, 1, 0);
    generate_straight_moves(board, rank, file, color, moves, -1, 0);
    generate_straight_moves(board, rank, file, color, moves, 0, 1);
    generate_straight_moves(board, rank, file, color, moves, 0, -1);
}

fn generate_bishop_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    generate_diagonal_moves(board, rank, file, color, moves, 1, 1);
    generate_diagonal_moves(board, rank, file, color, moves, 1, -1);
    generate_diagonal_moves(board, rank, file, color, moves, -1, 1);
    generate_diagonal_moves(board, rank, file, color, moves, -1, -1);
}

fn generate_queen_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    generate_straight_moves(board, rank, file, color, moves, 1, 0);
    generate_straight_moves(board, rank, file, color, moves, -1, 0);
    generate_straight_moves(board, rank, file, color, moves, 0, 1);
    generate_straight_moves(board, rank, file, color, moves, 0, -1);
    generate_diagonal_moves(board, rank, file, color, moves, 1, 1);
    generate_diagonal_moves(board, rank, file, color, moves, 1, -1);
    generate_diagonal_moves(board, rank, file, color, moves, -1, 1);
    generate_diagonal_moves(board, rank, file, color, moves, -1, -1);
}

fn generate_king_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    for i in -1..=1 {
        for j in -1..=1 {
            if i == 0 && j == 0 {
                continue;
            }
            let new_rank = (rank as i8 + i) as usize;
            let new_file = (file as i8 + j) as usize;
            if new_rank < 8 && new_file < 8 {
                if let Some(target_piece) = board.get_piece(new_rank, new_file) {
                    if target_piece.color != color {
                        moves.push((new_rank, new_file));
                    }
                }
                else{
                    moves.push((new_rank, new_file));
                }
            }
        }
    }
}

fn generate_knight_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>) {
    let possible_moves: [(i8, i8); 8] = [
        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
        (1, -2), (1, 2), (2, -1), (2, 1),
    ];

    for &(delta_rank, delta_file) in possible_moves.iter() {
        let new_rank = (rank as i8 + delta_rank) as usize;
        let new_file = (file as i8 + delta_file) as usize;

        if new_rank < 8 && new_file < 8 {
            if let Some(target_piece) = board.get_piece(new_rank, new_file) {
                if target_piece.color != color {
                    moves.push((new_rank, new_file));
                }
            }
            else{
                 moves.push((new_rank, new_file));
            }
        }
    }
}

fn generate_straight_moves(board: &Board, rank: usize, file: usize, color: Color, moves: &mut Vec<(usize, usize)>, rank_direction: i8, file_direction: i8) {
    let mut current_rank = rank as i8 + rank_direction;
    let mut current_file = file as i8 + file_direction;

    while current_rank >= 0 && current_rank < 8 && current_file >= 0 && current_file < 8 {
        let new_rank = current_rank as usize;
        let new_file = current_file as usize;
        if let Some(target_piece) = board.get_piece(new_rank, new_file) {
            if target_piece.color != color {
                moves.push((new_rank, new_file));
            }
            break;
        } else {
            moves.push((new_rank, new_file));
        }
        current_rank += rank_direction;
        current_file += file_direction;
    }
}

fn generate_diagonal_moves(
    board: &Board,
    rank: usize,
    file: usize,
    color: Color,
    moves: &mut Vec<(usize, usize)>,
    rank_direction: i8,
    file_direction: i8,
) {
    let mut current_rank = rank as i8 + rank_direction;
    let mut current_file = file as i8 + file_direction;

    while current_rank >= 0 && current_rank < 8 && current_file >= 0 && current_file < 8 {
        let new_rank = current_rank as usize;
        let new_file = current_file as usize;
        if let Some(target_piece) = board.get_piece(new_rank, new_file) {
            if target_piece.color != color {
                moves.push((new_rank, new_file));
            }
            break;
        } else {
            moves.push((new_rank, new_file));
        }
        current_rank += rank_direction;
        current_file += file_direction;
    }
}