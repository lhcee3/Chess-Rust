# Chess-Rust Backend Architecture

## Overview

A Rust-based chess server with a **local search engine** (no external AI API), serving
an interactive web UI over HTTP and exposing a REST API. Authentication is delegated
to **Firebase Auth (Google sign-in)** on the frontend; the backend verifies the
Firebase ID token on every privileged request and scopes game state per user.

Single-binary, in-memory state, no chess database. All AI work is done in-process
in Rust.

> **What changed from the previous plan:** the Groq API client and `llama-3.3-70b`
> model are gone. The AI is now a classical search: alpha-beta with iterative
> deepening, transposition table, MVV-LVA + killer + history move ordering, and a
> tapered piece-square-table evaluation. Groq is kept as an *optional* remote
> fallback (toggled by config) for the rare case the user wants a stronger opponent
> than the local engine can offer — but the default path is 100% offline.

## Project Layout

```
Chess-Rust/
├── .env                        # FIREBASE_PROJECT_ID, optional GROQ_API_KEY
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
        │   ├── auth.rs         # Firebase ID-token verification
        │   ├── engine/
        │   │   ├── mod.rs      # Re-exports + Searcher struct
        │   │   ├── eval.rs     # Tapered PST evaluation
        │   │   ├── search.rs   # Alpha-beta + iterative deepening + quiescence
        │   │   ├── tt.rs       # Transposition table (Zobrist-keyed)
        │   │   ├── zobrist.rs  # Zobrist hashing for positions
        │   │   ├── ordering.rs # MVV-LVA, killers, history, TT-move-first
        │   │   └── time.rs     # Time management / soft-hard limits
        │   └── (legacy stubs: engine.rs, uci.rs, groq_ai.rs removed)
        └── static/
            ├── index.html      # Web UI (chessboard + Firebase login)
            ├── app.js          # Frontend logic (move flow, AI trigger)
            ├── auth.js         # Firebase Auth (Google sign-in)
            ├── chessboard-1.0.0/  # chessboard.js library (vendored)
            └── styles.css
```

## Layer Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       Web Browser                            │
│   static/index.html + chessboard.js + Firebase JS SDK        │
│   (Google sign-in → ID token → attach as Authorization:      │
│    Bearer <jwt> on every /api/* call)                        │
└──────────────────────────┬───────────────────────────────────┘
                           │ HTTP + Bearer JWT
┌──────────────────────────▼───────────────────────────────────┐
│                         main.rs                              │
│   actix-web on 127.0.0.1:8080                                │
│   Middleware: auth (verify Firebase ID token)                │
│   State: AppState {                                          │
│     games: Mutex<HashMap<Uid, Game>>,  // per-user game      │
│     config: RwLock<Config>,             // engine tunables   │
│   }                                                          │
└──────┬──────────┬─────────────┬─────────────┬────────────────┘
       │          │             │             │
       │          │             │             │
┌──────▼────┐ ┌───▼────┐ ┌──────▼─────┐ ┌─────▼─────┐
│ auth.rs   │ │game.rs │ │ engine/    │ │ (optional)│
│ Firebase  │ │Rules   │ │  search.rs │ │ groq_ai   │
│ JWT verify│ │engine  │ │ alpha-beta │ │  .rs      │
│           │ │Legal   │ │ ID + quies │ │ (fallback)│
└───────────┘ │ moves  │ │ TT + order │ └───────────┘
              └───┬─────┘ └──────┬─────┘
                  │              │
              ┌───▼────┐   ┌─────▼──────┐
              │board.rs│   │ eval.rs    │
              │ Types  │   │ PST + mat  │
              └───┬────┘   └────────────┘
                  │
              ┌───▼──────┐
              │move_gen  │
              │.rs       │
              └──────────┘
```

## Dependencies (Cargo.toml)

| Crate | Version | Purpose |
|---|---|---|
| `actix-web` | 4.10 | Web framework |
| `actix-web-httpauth` | 0.8 | Bearer-token extractor |
| `tokio` | 1.45 | Async runtime (full features) |
| `serde` / `serde_json` | 1.x | JSON serialization |
| `firebase-auth` | 0.4 | Firebase ID-token verification (REST verify) |
| `reqwest` | 0.12 | HTTP client (rustls-tls, no OpenSSL) |
| `dashmap` | 6 | Concurrent per-user game map |
| `parking_lot` | 0.12 | Faster mutexes |
| `env_logger` | 0.11 | Logging |
| `log` | 0.4 | Logging facade |
| `dotenvy` | 0.15 | Load `.env` at startup |
| `once_cell` | 1.20 | Lazy statics (Zobrist tables) |
| `rand` | 0.8 | One-time Zobrist key generation (rng seeded at compile time) |

Frontend (vendored, no build step):
- `chessboard.js` 1.0.0 + `chess.js` 1.0.0-beta (FEN / SAN / move validation)
- Firebase JS SDK 10.x (modular, via CDN) — `firebase/auth`

## Module Details

### `board.rs` — Core Types

Unchanged from previous plan. Defines `PieceType`, `Color`, `Piece`, `Move`,
`CastlingRights`, `Board` (flat `[Option<Piece>; 64]`), plus the same helper
functions (`square_index`, `rank_and_file`, `square_to_algebraic`,
`algebraic_to_square`, `is_square_attacked`).

### `move_gen.rs` — Move Generation

Unchanged. Pseudo-legal move generation for every piece type including en passant
and castling. King-safety filtering happens in `game.rs`.

### `game.rs` — Game State & Rules Engine

Same as before, minus the Groq-specific fields:

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
    pub ai_side: Option<Color>,           // None = human-vs-human
    pub engine_config: EngineConfig,      // see below
}
```

```rust
pub struct EngineConfig {
    pub depth_limit: u8,                  // hard cap, default 8
    pub time_limit_ms: u64,               // soft cap, default 2000
    pub use_quiescence: bool,             // default true
    pub use_transposition_table: bool,    // default true
    pub use_remote_fallback: bool,        // default false (Groq off)
    pub remote_model: String,             // used only if fallback on
}
```

Methods are the same (`new`, `get_legal_moves`, `make_move`, `make_move_uci`,
`to_fen`, `from_fen`, `move_to_uci`, `get_game_state_json`).

### `auth.rs` — Firebase ID-Token Verification

The actix middleware on every `/api/*` route (except `/`) does:

1. Extract `Authorization: Bearer <jwt>`.
2. POST to `https://identitytoolkit.googleapis.com/v1/accounts:lookup` with the
   ID token, using a server-side API key (or the public cert route for full JWT
   verification — see `firebase-auth` crate).
3. Confirm `email_verified == true` and the `aud` matches our Firebase project.
4. Attach the verified `uid` (and `email`) to request extensions.

```rust
pub struct UserCtx { pub uid: String, pub email: String }
```

Routes then take `user: UserCtx` and look up `games[&user.uid]`. Anonymous
sessions are not allowed for `/api/move` etc. — the frontend must complete Google
sign-in before the board activates.

### `engine/` — Local Search

This is the new heart of the system. Every piece is keyed off a Zobrist hash
of the position so they can share state efficiently.

#### `zobrist.rs` — Zobrist Hashing

- 12 random 64-bit keys per piece-type × color × square (12 × 64 = 768 keys).
- 4 castling keys, 8 en-passant-file keys, 1 side-to-move key.
- Generated once at startup (`once_cell`) using `rand::thread_rng()`.
- `Position::hash() -> u64` recomputes incrementally in `make_move` / `unmake_move`
  (XOR the pieces moving, castling bit flip, ep square change, side toggle). No
  full-board rehash in the hot loop.

#### `tt.rs` — Transposition Table

A fixed-size (default 64 MB) bucket array of `TtEntry`:

```rust
pub struct TtEntry {
    pub key: u64,           // full Zobrist, to catch collisions
    pub depth: i8,          // ply depth stored
    pub score: i32,         // score from side-to-move's POV
    pub flag: TtFlag,       // Exact | LowerBound | UpperBound
    pub best_move: Option<Move>,
}
```

Two-way bucket replacement scheme (always replace if `depth >= stored.depth`).
Used both as a cache and as the source of the "TT move" for ordering.

#### `ordering.rs` — Move Ordering

When generating moves at a node, sort by descending priority:

1. **TT move** (if available, scored `+∞`).
2. **Winning captures** (MVV-LVA: victim value × 10 − attacker value, e.g.
   `P×Q = 1500`, `N×P = 210`).
3. **Equal captures** (also MVV-LVA).
4. **Killer moves** — two slots per ply, only for non-captures.
5. **History heuristic** — `history[from][to]` counter, incremented on beta
   cutoffs for quiet moves.
6. **Losing captures** (still MVV-LVA, but score low).
7. **Castling** gets a small bonus.
8. Everything else.

Static Exchange Evaluation (SEE) runs as a fast prefilter on captures to swap
"losing captures" further down the list (skipped at shallow depth for speed).

#### `eval.rs` — Position Evaluation

Tapered eval from side-to-move's POV, returned in centipawns.

**Material values (mg / eg):**

| Piece | MG | EG |
|---|---|---|
| Pawn | 100 | 120 |
| Knight | 320 | 300 |
| Bishop | 330 | 320 |
| Rook | 500 | 520 |
| Queen | 900 | 950 |
| King | 0 | 0 |

**Piece-Square Tables (PSTs):** standard tapered tables — bishops and knights
penalized on the rim, knights centered, rooks on 7th rank, king on 1st/2nd in MG
and centered in EG, pawns rewarded for advancement. Combined via a phase
function `phase = 4 * minor_left + 2 * rook_left + 1 * queen_left`, capped at 24.

**Additive terms (cheap, big payoff):**
- Bishop pair: +30 (MG) / +50 (EG).
- Doubled / isolated / backward pawn penalties.
- Open / half-open file bonuses for rooks.
- Passed-pawn bonuses, rank-scaled.
- King-safety shield pawn bonus in MG only.
- King-to-queen-file tropism for queens in MG only.

Tempo: +10 cp for side to move.

No NNUE, no PSQT learning — keeps the engine fully deterministic, instant to
build, and ~0.2 ms per node on a modern CPU.

#### `search.rs` — Search

```rust
pub fn best_move(game: &Game, cfg: &EngineConfig) -> Option<Move>
```

Driver:
1. **Iterative deepening**: depth 1 → 2 → … → `cfg.depth_limit`, stopping
   early if the soft time limit is hit. Each iteration prints (logs) the
   PV, depth, NPS, and score.
2. At each depth, call `alpha_beta(root, depth, −INF, +INF, ply=0)`.
3. Use the PV of the previous iteration as the **principal variation**, and
   search its first move first at the next iteration (PV-node ordering).

```rust
fn alpha_beta(pos: &mut Position, depth: i8, mut alpha: i32, beta: i32,
              ply: u8, ss: &mut SearchStack) -> i32
```

- **TT probe** at the top. If `entry.depth >= depth` and the bound is exact
  (or matches α/β), return `entry.score` directly.
- **Mate-distance pruning**: clamp α/β to `±(MATE − ply)`.
- **Reverse futility pruning (RFPR)** at shallow depth in non-PV nodes.
- **Null-move pruning (NMP)** when not in check, depth ≥ 3, and we have
  non-pawn material: pass the move with reduced depth, fail-high on β.
- **Futility pruning** at depth ≤ 2 near the leaves if static eval + margin
  is still below α.
- **Late-move reductions (LMR)** for quiet moves after the first few have
  failed high in non-PV nodes.
- **Quiescence search** at depth 0: only captures (and queen/rook/ bishop
  promotions), recurse on SEE ≥ 0 captures, return static eval when stable.

Perft-style correctness check at startup (depth 1–4) is logged so a regression
in move-gen is caught immediately.

```rust
pub struct SearchStack {
    pub killers: [[Option<Move>; 2]; MAX_PLY],
    pub history: [[i32; 64]; 64],
}
```

#### `time.rs` — Time Management

- Soft limit: `cfg.time_limit_ms`.
- Hard limit: `soft × 1.5` (panic-stop with the best move so far).
- Iterative-deepening driver checks the clock after every completed root child.

### Legacy modules removed

- `engine.rs` (the old "list all moves" stub) — **deleted**, superseded by
  `engine/`.
- `uci.rs` — **deleted**, dead code, never wired up.
- `groq_ai.rs` — **moved to `legacy/groq_ai.rs`**, compiled behind a
  `remote-fallback` feature flag. Default: off.

## API Endpoints

All `/api/*` routes require `Authorization: Bearer <firebase-id-token>`. The
token is verified by the `auth` middleware and the request handler receives a
`UserCtx`.

Base URL: `http://127.0.0.1:8080`

### `GET /` — Web UI

Returns the embedded `static/index.html`. If the user is not signed in, the page
shows a "Sign in with Google" button. The board is hidden until sign-in succeeds.

### `GET /api/game` — Game State (per user)

Returns the game belonging to the caller's UID. If none exists, auto-creates a
fresh game with the user's last-used config (or defaults: `ai_side=Black`,
`depth=8`, `time=2000`).

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
  "engine": { "depth": 8, "nodes": 0, "nps": 0, "scoreCp": 0, "pv": ["e2e4"] },
  "user": { "uid": "...", "email": "..." }
}
```

### `POST /api/move` — Human Move

```json
{ "uci": "e2e4" }
```

Returns the updated game state. Guards: game-over, AI's turn, move not legal.

### `POST /api/ai-move` — Trigger AI

No body. The server runs the local `engine::best_move(&game, &cfg)` and applies
the result. If `use_remote_fallback` is on and the local engine returns `None`
or throws, fall through to Groq.

```json
{ "ok": true, "uci": "e7e5", "engine": "local", "depth": 8, "nodes": 412053,
  "nps": 235000, "scoreCp": 18, "timeMs": 1754, "pv": ["e7e5","e2e4","..."] }
```

### `POST /api/new-game` — Reset (per user)

```json
{ "ai_side": "black", "depth": 8, "time_limit_ms": 2000 }
```

Resets the caller's game in place. All fields optional.

### `POST /api/config` — Runtime Config (per user)

```json
{ "ai_side": "white", "depth": 10, "time_limit_ms": 5000,
  "use_remote_fallback": false }
```

Persists to in-memory `EngineConfig` for the caller. No global mutation.

### `GET /api/health` — Unauthenticated

Returns `{ "ok": true, "version": "...", "engine": "local" }`. Used by the
frontend to decide whether to show the sign-in flow.

## State Management

```rust
struct AppState {
    games: DashMap<String, Game>,          // uid → game
    configs: DashMap<String, EngineConfig>, // uid → last config
    server_cfg: RwLock<ServerConfig>,      // shared, read-mostly
}
```

- One game per authenticated user. No cross-user reads/writes.
- Locking discipline: handlers take a short-lived read or write on the user's
  game, then drop the guard before calling into the engine (which doesn't need
  the lock at all — the engine only reads an immutable snapshot of the
  position via `Game::clone_for_search()`).
- For the AI move handler: clone the game, release the map guard, run search,
  re-acquire the guard to apply the chosen move (validating it's still legal
  after any concurrent human move — usually it isn't a concern since only one
  client per user, but defense-in-depth is cheap).

## Configuration

`.env` at project root (loaded by `dotenvy`):

```
FIREBASE_PROJECT_ID=your-firebase-project
FIREBASE_API_KEY=AIza...                  # server-side, used for token lookup
GROQ_API_KEY=gsk_...                      # only used if remote_fallback=true
DEFAULT_DEPTH=8
DEFAULT_TIME_MS=2000
TT_SIZE_MB=64
```

Per-user config (depth, time, side, fallback) lives in `configs[uid]` and is
mutable through `/api/config`. The defaults above seed new users on first
`/api/game`.

## Frontend (`static/`)

Plain HTML + vanilla JS + chessboard.js, no build step. Loaded directly from
the actix static-files route.

### `auth.js` — Firebase Auth (Google)

```js
import { initializeApp } from "https://www.gstatic.com/firebasejs/10.x/firebase-app.js";
import { getAuth, GoogleAuthProvider, signInWithPopup, onAuthStateChanged }
  from "https://www.gstatic.com/firebasejs/10.x/firebase-auth.js";

const app = initializeApp(firebaseConfig);
const auth = getAuth(app);
const provider = new GoogleAuthProvider();
```

- `onAuthStateChanged` gates the board UI. When the user is signed in, the
  `idToken` is fetched via `user.getIdToken()` and stored in memory (never
  localStorage — JWTs are short-lived and we always want a fresh one).
- A small `apiFetch(path, opts)` wrapper attaches `Authorization: Bearer ${idToken}`
  to every request and handles 401 by re-prompting sign-in.
- Sign-out button calls `auth.signOut()` and clears the in-memory game state.

### `app.js` — Game Flow

- On load: pull `/api/game` → render board with `chessboard.js` position.
- `chess.js` (vendored) keeps the canonical position on the client (legal-move
  filtering, SAN notation, captured-piece tracking).
- Click source square → highlight legal targets (green dots) by calling
  `/api/game` once and reading `legalMoves` to filter targets.
- Click target → POST `/api/move { uci }` → re-render.
- After every human move, if it's the AI's turn, POST `/api/ai-move` and animate
  the result on the board.
- Move history panel + captured-pieces tray driven by `getGameStateJson()`.
- Engine stats panel (depth / NPS / PV) populated from the `/api/ai-move`
  response.

### `index.html` — Structure

```
┌────────────────────────────────────────────┐
│  [Sign in with Google]   user@email.com  ⎋ │
├──────────────────────────────────┬─────────┤
│                                  │ History │
│          8×8 chessboard          │  1.e4   │
│         (chessboard.js)          │  1...e5 │
│                                  │  ...    │
├──────────────────────────────────┴─────────┤
│  Captured: ♟ ♟ ♙   Status: playing         │
│  Engine: depth 8, NPS 235k, score +0.18    │
│  PV: e7e5 e2e4 g1f3 ...                    │
└────────────────────────────────────────────┘
```

## Build & Run

```bash
# Build
cd Chess-Rust/chess_backend
CC="/tmp/zig-cc-wrapper.sh" \
RUSTFLAGS="-C linker=/tmp/zig-cc-wrapper.sh" \
cargo build --release

# Run
cd /path/to/Chess-Rust
RUST_LOG=info ./chess_backend/target/release/chess_backend

# Server starts at http://127.0.0.1:8080
# Open the URL, click "Sign in with Google", play.
```

## Build Constraints

- No system `gcc` — `zig cc` wrapper required (unchanged from previous plan).
- `reqwest` with `rustls-tls` — avoids OpenSSL.

## Tests

Unit tests for:
- Board init, piece placement, display, square helpers.
- `is_square_attacked` from starting position + a few FENs.
- `make_move` updates (castling rights, en passant, halfmove, promotion).
- Scholar's mate detection.
- FEN roundtrip.
- `move_gen` perft: depth 1 = 20, depth 2 = 400, depth 3 = 8902.
- **Engine**: perft matches `move_gen::perft` at depth 1–4.
- **TT**: store/retrieve/probe collisions, replacement scheme.
- **Ordering**: TT move first, MVV-LVA ordering for captures, killers persist
  per ply.
- **Search**: finds scholar's mate at depth ≤ 4; mate distance is exact.
- **Eval**: starting position ≈ 0.0 cp; up-a-pawn position ≈ +100 mg / +120 eg.
- **Auth**: rejects missing token, rejects expired token, accepts valid token
  (against a stub Firebase verifier or a recorded fixture).

Run:

```bash
CC="/tmp/zig-cc-wrapper.sh" RUSTFLAGS="-C linker=/tmp/zig-cc-wrapper.sh" cargo test
```

## Architecture Decisions

| Decision | Rationale |
|---|---|
| **Local alpha-beta engine** | No API key, no rate limit, no network dependency, fully offline-capable, sub-second responses for casual play. |
| **Iterative deepening** | Gives a usable move at any time cutoff; PV carries over as ordering hint. |
| **Transposition table** | Real positions recur frequently (especially with castling / promotion); cache hits save huge work. |
| **MVV-LVA + killers + history** | Cheap ordering tricks that yield the bulk of the speedup over plain alpha-beta. |
| **Quiescence search** | Prevents the horizon effect — engine won't sacrifice a queen because it can't "see" the recapture. |
| **Tapered PST eval** | Different piece values for middlegame vs endgame is the single biggest eval-quality win for ~20 lines of code. |
| **Firebase Auth (Google)** | Zero password storage, free, familiar UX. Server only sees the verified `uid`/`email`. |
| **Per-user `DashMap`** | Multiple signed-in users can each have their own game; lock-free reads for `/api/game` is fast. |
| **Drop Groq by default** | Matches the user's intent ("remove AI, not fully") — fallback flag keeps the option without the dependency. |
| **chessboard.js + chess.js (vendored)** | No npm build step; the backend is a single binary that serves the whole app. |
| **In-memory state** | Single-process, single-host; matches the previous deployment shape. If persistence is wanted later, swap `DashMap` for a `sled` / `sqlite` layer. |
| **Mutate `Game` in place during search** | Avoids per-node allocations; pairs with incremental Zobrist updates. |

## Roadmap (future, not in this iteration)

- `mcts` variant in the engine for fun / training.
- UCI loop (`uci.rs`) over WebSocket so external GUI can connect to the same
  engine binary.
- PGN import / export per game.
- Optional `sled`-backed game history keyed by `(uid, game_id)`.
- Per-user rating / ELO tracking once we have a second human.
- NNUE eval as a drop-in replacement for PST eval (gated by a feature flag).
