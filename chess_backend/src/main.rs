use std::sync::Mutex;
use actix_web::{web, App, HttpResponse, HttpServer, middleware};
use serde::{Deserialize, Serialize};
use serde_json::json;
use chess_backend::game::{Game, GameStatus};
use chess_backend::engine::{best_move, analyze};

struct AppState {
    game: Mutex<Game>,
}

#[derive(Deserialize)]
struct MoveRequest {
    uci: String,
}

#[derive(Deserialize)]
struct GameConfigRequest {
    engine_side: Option<String>,
    depth: Option<u8>,
    time_limit_ms: Option<u64>,
    analysis_depth: Option<u8>,
    analysis_time_limit_ms: Option<u64>,
}

#[derive(Deserialize)]
struct AnalyzeRequest {
    #[allow(dead_code)]
    game_id: Option<String>,
    fen_before: String,
    uci: String,
    depth: Option<u8>,
    time_limit_ms: Option<u64>,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    message: Option<String>,
    error: Option<String>,
}

fn bad(msg: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse {
        ok: false, message: None, error: Some(msg.to_string()),
    })
}

fn apply_engine_side(game: &mut Game, side: Option<&str>) {
    if let Some(side) = side {
        game.engine_side = match side {
            "white" => Some(chess_backend::board::Color::White),
            "black" => Some(chess_backend::board::Color::Black),
            "none" => None,
            _ => game.engine_side,
        };
    }
}

fn apply_config(game: &mut Game, body: &GameConfigRequest) {
    apply_engine_side(game, body.engine_side.as_deref());
    if let Some(d) = body.depth { game.engine_config.move_depth = d; }
    if let Some(t) = body.time_limit_ms { game.engine_config.move_time_limit_ms = t; }
    if let Some(d) = body.analysis_depth { game.engine_config.analysis_depth = d; }
    if let Some(t) = body.analysis_time_limit_ms { game.engine_config.analysis_time_limit_ms = t; }
}

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/index.html"))
}

async fn get_game(data: web::Data<AppState>) -> HttpResponse {
    let game = data.game.lock().unwrap();
    HttpResponse::Ok().json(game.get_game_state_json())
}

/// Applies the human's move and returns immediately. No engine search and no
/// analysis runs on this path, so the player's move is never blocked.
async fn make_move(data: web::Data<AppState>, body: web::Json<MoveRequest>) -> HttpResponse {
    let mut game = data.game.lock().unwrap();

    if matches!(game.status, GameStatus::Checkmate(_) | GameStatus::Stalemate) {
        return bad("Game is over");
    }
    if let Some(engine) = game.engine_side {
        if engine == game.side_to_move {
            return bad("It's the engine's turn");
        }
    }

    match game.make_move_uci(&body.uci) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            ok: true, message: Some(body.uci.clone()), error: None,
        }),
        Err(e) => bad(&e),
    }
}

/// Chooses and applies the engine's reply. The game mutex is held only to take a
/// snapshot and, once the search is done, to verify + apply. The expensive search
/// runs on a blocking thread with the lock released.
async fn engine_move(data: web::Data<AppState>) -> HttpResponse {
    let (snapshot, cfg, fen_before) = {
        let game = data.game.lock().unwrap();
        if matches!(game.status, GameStatus::Checkmate(_) | GameStatus::Stalemate) {
            return bad("Game is over");
        }
        match game.engine_side {
            Some(engine) if engine == game.side_to_move => {}
            Some(_) => return bad("It's not the engine's turn"),
            None => return bad("The engine is not playing"),
        }
        (game.clone(), game.engine_config.clone(), game.to_fen())
    };

    let result = match tokio::task::spawn_blocking(move || best_move(&snapshot, &cfg)).await {
        Ok(r) => r,
        Err(e) => return HttpResponse::InternalServerError().json(ApiResponse {
            ok: false, message: None, error: Some(format!("Search task failed: {}", e)),
        }),
    };

    let mut game = data.game.lock().unwrap();
    if game.to_fen() != fen_before {
        return bad("Position changed while the engine was thinking");
    }

    if let Some(mv) = result.best_move {
        let uci = game.move_to_uci(&mv);
        match game.make_move(&mv) {
            Ok(()) => HttpResponse::Ok().json(json!({
                "ok": true,
                "uci": uci,
                "engine": "local",
                "depth": result.depth,
                "nodes": result.nodes,
                "nps": result.nps,
                "scoreCp": result.score,
                "timeMs": result.time_ms,
            })),
            Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
                ok: false, message: None,
                error: Some(format!("Engine move failed: {}", e)),
            }),
        }
    } else {
        HttpResponse::Ok().json(json!({
            "ok": true,
            "uci": null,
            "message": "No move found (stalemate or checkmate)",
        }))
    }
}

/// Reviews the human's move against the best alternatives. Reconstructs the
/// position from `fen_before` on a throwaway game and never touches the live
/// game, so it can run for longer without affecting normal play.
async fn analyze_move(data: web::Data<AppState>, body: web::Json<AnalyzeRequest>) -> HttpResponse {
    let mut cfg = { data.game.lock().unwrap().engine_config.clone() };
    if let Some(d) = body.depth { cfg.analysis_depth = d; }
    if let Some(t) = body.time_limit_ms { cfg.analysis_time_limit_ms = t; }

    let mut game = Game::new();
    if let Err(e) = game.from_fen(&body.fen_before) {
        return bad(&format!("Invalid fen_before: {}", e));
    }
    let uci = body.uci.clone();

    let result = match tokio::task::spawn_blocking(move || analyze(&game, &uci, &cfg)).await {
        Ok(r) => r,
        Err(e) => return HttpResponse::InternalServerError().json(ApiResponse {
            ok: false, message: None, error: Some(format!("Analysis task failed: {}", e)),
        }),
    };

    match result {
        Some(a) => HttpResponse::Ok().json(json!({
            "ok": true,
            "uci": a.played_uci,
            "classification": a.classification,
            "playedScoreCp": a.played_score_cp,
            "bestScoreCp": a.best_score_cp,
            "lossCp": a.loss_cp,
            "bestMove": a.best_move,
            "alternatives": a.alternatives,
            "reasonCodes": a.reason_codes,
            "engineDepth": a.engine_depth,
            "nodes": a.nodes,
            "nps": a.nps,
            "timeMs": a.time_ms,
        })),
        None => HttpResponse::Ok().json(json!({
            "ok": false,
            "uci": body.uci,
            "error": "Move could not be analyzed in this position",
        })),
    }
}

async fn new_game(data: web::Data<AppState>, body: web::Json<GameConfigRequest>) -> HttpResponse {
    let mut game = Game::new();
    apply_config(&mut game, &body);
    *data.game.lock().unwrap() = game;
    HttpResponse::Ok().json(ApiResponse { ok: true, message: None, error: None })
}

async fn config(data: web::Data<AppState>, body: web::Json<GameConfigRequest>) -> HttpResponse {
    let mut game = data.game.lock().unwrap();
    apply_config(&mut game, &body);
    HttpResponse::Ok().json(ApiResponse { ok: true, message: None, error: None })
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "ok": true,
        "version": "0.2.0",
        "engine": "local",
        "auth": "disabled",
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let data = web::Data::new(AppState {
        game: Mutex::new(Game::new()),
    });

    log::info!("Starting chess server on http://0.0.0.0:8080");
    log::info!("Engine: local alpha-beta (move depth 8/2s, analysis depth 5/5s)");
    log::info!("Mode: local play (no auth, single game)");

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(index))
            .route("/api/health", web::get().to(health))
            .service(
                web::scope("/api")
                    .route("/game", web::get().to(get_game))
                    .route("/move", web::post().to(make_move))
                    .route("/engine-move", web::post().to(engine_move))
                    .route("/analyze-move", web::post().to(analyze_move))
                    .route("/new-game", web::post().to(new_game))
                    .route("/config", web::post().to(config))
            )
    })
    .bind("0.0.0.0:8080")?
    .bind("0.0.0.0:8090")?
    .run()
    .await
}
