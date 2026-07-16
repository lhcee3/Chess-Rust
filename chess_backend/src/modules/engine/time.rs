use std::time::Instant;

pub struct TimeManager {
    start: Instant,
    soft_limit_ms: u64,
    hard_limit_ms: u64,
}

impl TimeManager {
    pub fn new(time_limit_ms: u64) -> Self {
        TimeManager {
            start: Instant::now(),
            soft_limit_ms: time_limit_ms,
            hard_limit_ms: (time_limit_ms as f64 * 1.5) as u64,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn soft_exceeded(&self) -> bool {
        self.elapsed_ms() >= self.soft_limit_ms
    }

    pub fn hard_exceeded(&self) -> bool {
        self.elapsed_ms() >= self.hard_limit_ms
    }
}
