# Chess-Rust Backend Architecture

## Overview

A Rust-based chess server with AI opponent powered by Groq API (`llama-3.3-70b-versatile`). Single-binary, in-memory state, no database, no auth. Serves an interactive web UI over HTTP and exposes a REST API.

## Project Layout

```
Chess-Rust/
├── .env                        # GROQ_API_KEY (loaded at startup)
├── .gitignore
├── ARCHITECTURE.md
└── chess_backend/
    ├── Cargo.toml
    └── src/
        ├── main.rs             # HTTP server entry point
        ├── lib.rs              # Module re-exports
        ├── modules/
        │   ├── mod.rs          # Module declarations
        │   ├── board.rs        # Board, Piece, Move types + helpers
        │   ├── move_gen.rs     # Pseudo-legal move generation
        │   ├── game.rs         # Game state, legal moves, rules engine
        │   ├── engine.rs       # Simple engine (move listing)
        │   ├── groq_ai.rs      # Groq API client
        │   └── uci.rs          # UCI protocol (unused / stub)
        └── static/
            └── index.html      # Web UI embedded in binary
```

## Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Web Browser                          │
│              (static/index.html - chess UI)              │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP
┌──────────────────────▼──────────────────────────────────┐
│                   main.rs                                │
│    actix-web server bound to 127.0.0.1:8080              │
│    Routes: / (HTML), /api/game, /api/move, etc.          │
│    State: AppState { game: Mutex<Game>, api_key: Mutex }  │
└──────────────────────┬──────────────────────────────────┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
┌────────▼──────┐ ┌────▼────┐ ┌─────▼────────┐
│   game.rs     │ │groq_ai │ │    lib.rs     │
│ Rules engine  │ │ .rs    │ │  Re-exports   │
│ Legal moves   │ │ AI     │ │               │
│ Check/mate    │ │ client │ │               │
│ FEN generation│ │        │ │               │
└───────┬───────┘ └────────┘ └───────────────┘
        │
   ┌────▼────┐
   │ board   │
   │ .rs     │
   │ Types   │
   └────┬────┘
        │
   ┌────▼──────┐
   │move_gen   │
   │.rs        │
   │Per-piece  │
   │move gen   │
   └───────────┘
```

## Dependencies (Cargo.toml)

| Crate | Version | Purpose |
|---|---|---|
| `actix-web` | 4.10.2 | Web framework |
| `actix` | 0.13 | Actor framework (used by uci.rs) |
| `tokio` | 1.45 | Async runtime (full features) |
| `serde` / `serde_json` | 1.x | JSON serialization |
| `reqwest` | 0.12 | HTTP client to Groq API (rustls-tls, no OpenSSL) |
| `env_logger` | 0.11 | Logging |
| `log` | 0.4 | Logging facade |
| `dotenvy` | 0.15 | Load `.env` at startup |

## Module Details

### `board.rs` — Core Types

Defines the fundamental chess types:

- **`PieceType`**: `Pawn`, `Knight`, `Bishop`, `Rook`, `Queen`, `King` (with `to_char`/`from_char` for FEN)
- **`Color`**: `White`, `Black` (with `opposite()`)
- **`Piece`**: `{ piece_type: PieceType, color: Color }`
- **`Move`**: `{ from: usize, to: usize, captured: Option<PieceType>, promotion: Option<PieceType>, is_castle: bool }`
- **`CastlingRights`**: bitfield `[K, Q, k, q]`
- **`Board`**: `{ pieces: [Option<Piece>; 64] }` — flat array indexed as `rank * 8 + file`

Key helper functions:

| Function | Purpose |
|---|---|
| `square_index(rank, file)` | Convert (rank, file) → index |
| `rank_and_file(sq)` | Convert index → (rank, file) |
| `square_to_algebraic(sq)` | e.g., 0 → "a1" |
| `algebraic_to_square(s)` | e.g., "e4" → 28 |
| `is_square_attacked(board, sq, by_color)` | Does any piece of `by_color` attack `sq`? |

### `move_gen.rs` — Move Generation

Generates pseudo-legal moves for a piece at a given square:

- **`generate_moves(board, from, color, ep_target, castling_rights) -> Vec<Move>`**
- Handles: pawn pushes/captures/promotions/en passant, knight moves, bishop/rook/queen sliding, king moves, castling
- Does **not** filter for king safety (that's `game.rs`'s job)

### `game.rs` — Game State & Rules Engine

The central module. The **`Game`** struct holds all game state:

```rust
pub struct Game {
    pub board: Board,
    pub side_to_move: Color,
    pub castling_rights: CastlingRights,
    pub en_passant_target: Option<usize>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
    pub move_history: Vec<Move>,
    pub status: GameStatus,
    pub ai_side: Option<Color>,
    pub groq_model: String,
}
```

**`GameStatus`** enum: `Playing`, `Check`, `Checkmate(Color)`, `Stalemate`

Key methods:

| Method | Purpose |
|---|---|
| `new()` | Starting position |
| `get_legal_moves()` | Filters `move_gen::generate_moves` for king safety |
| `make_move(mv)` | Executes a move, updates board/rights/ep/clock, detects check/mate/stalemate |
| `make_move_uci(uci)` | Parse UCI string → validate vs legal → execute |
| `to_fen()` | Generate FEN string of current position |
| `from_fen(fen)` | Load position from FEN |
| `move_to_uci(mv)` | Convert `Move` → UCI string (e.g. "e7e8q") |
| `get_game_state_json()` | Full game state as JSON for the API |

**Legal move filtering**: For each pseudo-legal move, temporarily apply it to a copy of the board and check if the king is in check afterward. If not, it's legal.

### `groq_ai.rs` — AI Client

- **`get_ai_move(game, api_key)`**: Sends position FEN + legal moves list to Groq API
- **Prompt**: System prompt ("you are a UCI chess engine") + FEN + side + legal moves → respond with `bestmove <uci>`
- **Response parsing**: Extracts `bestmove` token, validates against legal moves
- **`get_ai_move_with_retry(game, api_key, max_retries)`**: Retry 3 times with 500ms delay

Groq endpoint: `POST https://api.groq.com/openai/v1/chat/completions`
Model: `llama-3.3-70b-versatile` (configurable via `Game.groq_model`)

### `engine.rs` — Stub

Simple `ChessEngine::generate_legal_moves()` that iterates all squares and aggregates `move_gen` output. Not used by the server — exists as skeleton for future local engine.

### `uci.rs` — Stub

WebSocket actor + `UciEngine` struct for spawning a UCI-compatible external engine process. Currently unused (compiles but is dead code). Leftover from earlier architecture.

## API Endpoints

All under `http://127.0.0.1:8080`

### `GET /` — Web UI

Returns the embedded `static/index.html` (CSS grid chessboard with Unicode pieces).

### `GET /api/game` — Game State

```json
{
  "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  "sideToMove": "white",
  "status": "playing",
  "winner": null,
  "legalMoves": ["e2e4", "d2d4", ...],
  "moveHistory": [],
  "aiSide": "black",
  "myTurn": true,
  "apiKeyConfigured": true
}
```

### `POST /api/move` — Human Move

**Request:** `{ "uci": "e2e4" }`
**Response:** `{ "ok": true, "message": "e2e4", "error": null }`

Guards: game over, AI's turn.

### `POST /api/ai-move` — Trigger AI

No request body. Calls Groq, applies the returned move.

**Response:** `{ "ok": true, "uci": "e7e5" }`

Guards: game over, not AI's turn, API key configured.

### `POST /api/new-game` — Reset

**Request:** `{ "ai_side": "black", "groq_model": "..." }`

Resets board to starting position. Fields are optional (keeps previous values if omitted).

### `POST /api/config` — Runtime Config

**Request:** `{ "api_key": "...", "ai_side": "black", "groq_model": "..." }`

Updates config at runtime without restart.

## State Management

```rust
struct AppState {
    game: Mutex<Game>,
    api_key: Mutex<String>,
}

// created once, shared via actix-web's web::Data
let data = web::Data::new(AppState { ... });
```

- **`game`**: Single `Game` behind `Mutex`. Only one player at a time.
- **`api_key`**: Groq API key, settable at runtime via `/api/config` or from `.env` at startup.
- **Cloning for AI**: The game is cloned before AI inference (release the lock first) to avoid holding the lock during HTTP call.

## Configuration

- **`.env` file** at project root: `GROQ_API_KEY=gsk_...` loaded via `dotenvy::dotenv().ok()`
- **`AI_SIDE`**: Default `Some(Color::Black)` — AI plays black. Configurable per-game.
- **`GROQ_MODEL`**: Default `"llama-3.3-70b-versatile"` set in `Game::new()`

## Web UI (`static/index.html`)

Embedded into the binary at compile time via `include_str!("../static/index.html")`.

- CSS grid board (8×8)
- Unicode piece characters
- Click-to-select source square, click-to-move to target
- Legal move highlights (green dots on valid targets)
- Automatic AI trigger after human move
- Move history panel
- Captured pieces display
- Settings panel (AI side, model, API key)
- Game status display (check, checkmate, stalemate)

## Build & Run

```bash
# Build (requires zig cc as C compiler on systems without gcc)
CC="/tmp/zig-cc-wrapper.sh" RUSTFLAGS="-C linker=/tmp/zig-cc-wrapper.sh" cargo build --release

# Run
cd /path/to/Chess-Rust
RUST_LOG=info ./chess_backend/target/release/chess_backend

# Server starts at http://127.0.0.1:8080
```

## Build Constraints

- **No system `gcc`**: Requires `zig cc` as C compiler wrapper (`/tmp/zig-cc-wrapper.sh`)
- **`reqwest` with `rustls-tls`**: Avoids OpenSSL system dependency

## Tests

17 unit tests covering:

- Board initialization, piece placement, display
- Square helper functions (index ↔ algebraic conversion)
- `is_square_attacked` from starting position
- Color opposite, piece type char roundtrip, castling rights
- Engine move generation
- Game initialization, legal moves (starting position), illegal move rejection
- Make move state updates, castling, en passant
- Scholar's Mate checkmate detection
- Initial FEN generation

Run: `CC="/tmp/zig-cc-wrapper.sh" RUSTFLAGS="-C linker=/tmp/zig-cc-wrapper.sh" cargo test`

## Architecture Decisions

| Decision | Rationale |
|---|---|
| **In-memory state** | Single-user local server; no persistence needed |
| **`Mutex<Game>`** | Simple, sufficient for single concurrent request |
| **Clone game for AI** | Don't hold lock during external HTTP call (could be seconds) |
| **Embedded HTML** | No separate file serving; single binary deployment |
| **`rustls-tls`** | Avoid OpenSSL system dependency (not available on target) |
| **`zig cc` wrapper** | No system C compiler available in build environment |
| **actix-web** | High performance, mature Rust web framework |
| **Groq API** | Fast inference via llama-3.3-70b-versatile; no local GPU needed |
