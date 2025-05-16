pub mod modules;

pub mod board {
    pub use crate::modules::board::*;
}
pub mod move_gen {
    pub use crate::modules::move_gen::*;
}
pub mod engine {
    pub use crate::modules::engine::*;
}
pub mod uci {
    pub use crate::modules::uci::*;
}