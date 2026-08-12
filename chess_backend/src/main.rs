use std::sync::Arc;
use actix_web::{web, App, HttpResponse, HttpServer, HttpRequest, middleware};
use serde::{Deserialize, Serialize};
use dashmap::DashMap;
use chess_backend::game::{Game, GameStatus};
use chess_backend::engine::{EngineConfig, best_move};
use chess_backend::auth::{FirebaseConfig, verify_token, UserCtx};

struct AppState {
    games: DashMap<String, Game>,
    firebase: FirebaseConfig,
}

#[derive(Deserialize)]
struct MoveRequest {
    uci: String,
}

#[derive(Deserialize)]
struct NewGameRequest {
    ai_side: Option<String>,
    depth: Option<u8>,
    time_limit_ms: Option<u64>,
}

#[derive(Deserialize)]
struct ConfigRequest {
    ai_side: Option<String>,
    depth: Option<u8>,
    time_limit_ms: Option<u64>,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    message: Option<String>,
    error: Option<String>,
}

async fn get_token(req: &HttpRequest) -> Result<String, HttpResponse> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| HttpResponse::Unauthorized().json(ApiResponse {
            ok: false, message: None,
            error: Some("Missing Authorization header".to_string()),
        }))?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| HttpResponse::Unauthorized().json(ApiResponse {
            ok: false, message: None,
            error: Some("Invalid Authorization format".to_string()),
        }))?;
    Ok(token.to_string())
}

async fn auth_middleware(req: &HttpRequest, data: &web::Data<AppState>) -> Result<UserCtx, HttpResponse> {
    if chess_backend::auth::dev_mode_enabled() {
        log::debug!("dev mode: no auth required");
        return Ok(UserCtx { uid: "local".to_string(), email: String::new() });
    }
    let token = get_token(req).await?;
    verify_token(&token, &data.firebase).await.map_err(|e| {
        HttpResponse::Unauthorized().json(ApiResponse {
            ok: false, message: None,
            error: Some(format!("Auth failed: {}", e)),
        })
    })
}

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/index.html"))
}

async fn get_game(data: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let user = match auth_middleware(&req, &data).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut game = data.games.entry(user.uid.clone()).or_insert_with(|| Game::new());
    let mut json = game.get_game_state_json();
    if let Some(map) = json.as_object_mut() {
        map.insert("user".to_string(), serde_json::json!({
            "uid": user.uid,
            "email": user.email,
        }));
    }
    HttpResponse::Ok().json(json)
}

async fn make_move(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<MoveRequest>,
) -> HttpResponse {
    let user = match auth_middleware(&req, &data).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut game = data.games.entry(user.uid.clone()).or_insert_with(|| Game::new());

    if matches!(game.status, GameStatus::Checkmate(_) | GameStatus::Stalemate) {
        return HttpResponse::BadRequest().json(ApiResponse {
            ok: false, message: None,
            error: Some("Game is over".to_string()),
        });
    }
    if let Some(ai) = game.ai_side {
        if ai == game.side_to_move {
            return HttpResponse::BadRequest().json(ApiResponse {
                ok: false, message: None,
                error: Some("It's the AI's turn".to_string()),
            });
        }
    }

    match game.make_move_uci(&body.uci) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            ok: true, message: Some(body.uci.clone()), error: None,
        }),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse {
            ok: false, message: None, error: Some(e),
        }),
    }
}

async fn ai_move(data: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let user = match auth_middleware(&req, &data).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut game = data.games.entry(user.uid.clone()).or_insert_with(|| Game::new());

    if matches!(game.status, GameStatus::Checkmate(_) | GameStatus::Stalemate) {
        return HttpResponse::BadRequest().json(ApiResponse {
            ok: false, message: None,
            error: Some("Game is over".to_string()),
        });
    }
    if let Some(ai) = game.ai_side {
        if ai != game.side_to_move {
            return HttpResponse::BadRequest().json(ApiResponse {
                ok: false, message: None,
                error: Some("It's not the AI's turn".to_string()),
            });
        }
    }

    let cfg = game.engine_config.clone();
    let game_snapshot = game.clone();
    let result = best_move(&game_snapshot, &cfg);

    if let Some(mv) = result.best_move {
        let uci = game.move_to_uci(&mv);
        match game.make_move(&mv) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({
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
                error: Some(format!("AI move failed: {}", e)),
            }),
        }
    } else {
        HttpResponse::Ok().json(serde_json::json!({
            "ok": true,
            "uci": null,
            "message": "No move found (stalemate or checkmate)",
        }))
    }
}

async fn new_game(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<NewGameRequest>,
) -> HttpResponse {
    let user = match auth_middleware(&req, &data).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut game = Game::new();
    game.ai_side = match body.ai_side.as_deref() {
        Some("white") => Some(chess_backend::board::Color::White),
        Some("black") => Some(chess_backend::board::Color::Black),
        Some("none") => None,
        _ => game.ai_side,
    };
    if let Some(d) = body.depth { game.engine_config.depth_limit = d; }
    if let Some(t) = body.time_limit_ms { game.engine_config.time_limit_ms = t; }
    data.games.insert(user.uid.clone(), game);
    HttpResponse::Ok().json(ApiResponse { ok: true, message: None, error: None })
}

async fn config(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ConfigRequest>,
) -> HttpResponse {
    let user = match auth_middleware(&req, &data).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut game = data.games.entry(user.uid.clone()).or_insert_with(|| Game::new());
    if let Some(ref side) = body.ai_side {
        game.ai_side = match side.as_str() {
            "white" => Some(chess_backend::board::Color::White),
            "black" => Some(chess_backend::board::Color::Black),
            "none" => None,
            _ => game.ai_side,
        };
    }
    if let Some(d) = body.depth { game.engine_config.depth_limit = d; }
    if let Some(t) = body.time_limit_ms { game.engine_config.time_limit_ms = t; }
    HttpResponse::Ok().json(ApiResponse { ok: true, message: None, error: None })
}

async fn health() -> HttpResponse {
    let auth = if chess_backend::auth::dev_mode_enabled() {
        "optional"
    } else {
        "required"
    };
    HttpResponse::Ok().json(serde_json::json!({
        "ok": true,
        "version": "0.2.0",
        "engine": "local",
        "auth": auth,
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let firebase = FirebaseConfig::new();
    if let Ok(key) = std::env::var("FIREBASE_API_KEY") {
        *firebase.api_key.write().unwrap() = key;
    }
    if let Ok(pid) = std::env::var("FIREBASE_PROJECT_ID") {
        *firebase.project_id.write().unwrap() = pid;
    }

    let data = web::Data::new(AppState {
        games: DashMap::new(),
        firebase,
    });

    log::info!("Starting chess server on http://127.0.0.1:8080");
    log::info!("Engine: local alpha-beta (depth 8, 2s)");
    log::info!("Set FIREBASE_API_KEY and FIREBASE_PROJECT_ID in .env for auth");

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
                    .route("/ai-move", web::post().to(ai_move))
                    .route("/new-game", web::post().to(new_game))
                    .route("/config", web::post().to(config))
            )
    })
    .bind("127.0.0.1:8080")?
    .bind("127.0.0.1:8090")?
    .run()
    .await
}
