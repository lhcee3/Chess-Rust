use crate::board::Move;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtFlag {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Debug, Clone, Copy)]
pub struct TtEntry {
    pub key: u64,
    pub depth: i8,
    pub score: i32,
    pub flag: TtFlag,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    entries: Vec<TtEntry>,
    mask: usize,
    pub hits: u64,
    pub misses: u64,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let num_entries = (size_mb * 1024 * 1024 / size_of::<TtEntry>()).next_power_of_two();
        let mask = num_entries - 1;
        let entries = vec![TtEntry {
            key: 0, depth: 0, score: 0, flag: TtFlag::Exact, best_move: None,
        }; num_entries];
        TranspositionTable { entries, mask, hits: 0, misses: 0 }
    }

    pub fn probe(&mut self, key: u64, depth: i8, alpha: i32, beta: i32) -> (bool, Option<i32>, Option<Move>) {
        let idx = key as usize & self.mask;
        let entry = &self.entries[idx];
        if entry.key == key {
            self.hits += 1;
            if entry.depth >= depth {
                let score = tt_score(entry.score, 0);
                match entry.flag {
                    TtFlag::Exact => return (true, Some(score), entry.best_move),
                    TtFlag::LowerBound if score >= beta => return (true, Some(score), entry.best_move),
                    TtFlag::UpperBound if score <= alpha => return (true, Some(score), entry.best_move),
                    _ => {}
                }
            }
            (false, None, entry.best_move)
        } else {
            self.misses += 1;
            (false, None, None)
        }
    }

    pub fn store(&mut self, key: u64, depth: i8, score: i32, flag: TtFlag, best_move: Option<Move>) {
        let idx = key as usize & self.mask;
        let entry = &mut self.entries[idx];
        if entry.depth <= depth {
            *entry = TtEntry { key, depth, score, flag, best_move };
        }
    }
}

fn tt_score(score: i32, ply: i32) -> i32 {
    if score > 900_000 {
        score - ply
    } else if score < -900_000 {
        score + ply
    } else {
        score
    }
}
