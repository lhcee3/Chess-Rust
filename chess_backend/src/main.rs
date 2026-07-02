use std::sync::Mutex;
use actix_web::{web, App, HttpResponse, HttpServer, middleware};
use serde::{Deserialize, Serialize};
use chess_backend::game::{Game, GameStatus};

struct AppState {
    game: Mutex<Game>,
    api_key: Mutex<String>,
}

#[derive(Deserialize)]
struct MoveRequest {
    uci: String,
}

#[derive(Deserialize)]
struct NewGameRequest {
    ai_side: Option<String>,
    groq_model: Option<String>,
}

#[derive(Deserialize)]
struct ConfigRequest {
    api_key: Option<String>,
    ai_side: Option<String>,
    groq_model: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse {
    ok: bool,
    message: Option<String>,
    error: Option<String>,
}

async fn get_game(data: web::Data<AppState>) -> HttpResponse {
    let game = data.game.lock().unwrap();
    let mut json = game.get_game_state_json();
    if let Some(map) = json.as_object_mut() {
        let has_key = !data.api_key.lock().unwrap().is_empty() ||
                       std::env::var("GROQ_API_KEY").is_ok();
        map.insert("apiKeyConfigured".to_string(), serde_json::json!(has_key));
    }
    HttpResponse::Ok().json(json)
}

async fn make_move(
    data: web::Data<AppState>,
    body: web::Json<MoveRequest>,
) -> HttpResponse {
    let mut game = data.game.lock().unwrap();

    if game.status == GameStatus::Checkmate(_) || game.status == GameStatus::Stalemate {
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
            ok: true,
            message: Some(body.uci.clone()),
            error: None,
        }),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse {
            ok: false, message: None,
            error: Some(e),
        }),
    }
}

async fn ai_move(data: web::Data<AppState>) -> HttpResponse {
    let api_key = {
        let key = data.api_key.lock().unwrap().clone();
        if !key.is_empty() {
            key
        } else {
            match std::env::var("GROQ_API_KEY") {
                Ok(k) => k,
                Err(_) => return HttpResponse::InternalServerError().json(ApiResponse {
                    ok: false, message: None,
                    error: Some("GROQ_API_KEY not set. Set it as env var or via /api/config".to_string()),
                }),
            }
        }
    };

    let game_clone = {
        let game = data.game.lock().unwrap();
        if let Some(ai) = game.ai_side {
            if ai != game.side_to_move {
                return HttpResponse::BadRequest().json(ApiResponse {
                    ok: false, message: None,
                    error: Some("It's not the AI's turn".to_string()),
                });
            }
        }
        if game.status == GameStatus::Checkmate(_) || game.status == GameStatus::Stalemate {
            return HttpResponse::BadRequest().json(ApiResponse {
                ok: false, message: None,
                error: Some("Game is over".to_string()),
            });
        }
        game.clone()
    };

    let result = chess_backend::groq_ai::get_ai_move_with_retry(&game_clone, &api_key, 3).await;

    match result {
        Ok(uci) => {
            let mut game = data.game.lock().unwrap();
            match game.make_move_uci(&uci) {
                Ok(()) => HttpResponse::Ok().json(serde_json::json!({
                    "ok": true,
                    "uci": uci,
                })),
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
                    ok: false, message: None,
                    error: Some(format!("AI move '{}' failed: {}", uci, e)),
                }),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
            ok: false, message: None,
            error: Some(e),
        }),
    }
}

async fn new_game(
    data: web::Data<AppState>,
    body: web::Json<NewGameRequest>,
) -> HttpResponse {
    let mut game = data.game.lock().unwrap();
    *game = Game::new();
    game.ai_side = match body.ai_side.as_deref() {
        Some("white") => Some(chess_backend::board::Color::White),
        Some("black") => Some(chess_backend::board::Color::Black),
        Some("none") => None,
        _ => game.ai_side,
    };
    if let Some(ref model) = body.groq_model {
        game.groq_model = model.clone();
    }
    HttpResponse::Ok().json(game.get_game_state_json())
}

async fn config(
    data: web::Data<AppState>,
    body: web::Json<ConfigRequest>,
) -> HttpResponse {
    if let Some(ref key) = body.api_key {
        *data.api_key.lock().unwrap() = key.clone();
    }
    if let Some(ref side) = body.ai_side {
        let mut game = data.game.lock().unwrap();
        game.ai_side = match side.as_str() {
            "white" => Some(chess_backend::board::Color::White),
            "black" => Some(chess_backend::board::Color::Black),
            "none" => None,
            _ => game.ai_side,
        };
    }
    if let Some(ref model) = body.groq_model {
        let mut game = data.game.lock().unwrap();
        game.groq_model = model.clone();
    }
    HttpResponse::Ok().json(ApiResponse { ok: true, message: None, error: None })
}

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/index.html"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let data = web::Data::new(AppState {
        game: Mutex::new(Game::new()),
        api_key: Mutex::new(String::new()),
    });

    log::info!("Starting chess server on http://127.0.0.1:8080");
    log::info!("Set GROQ_API_KEY environment variable or use /api/config for AI moves");

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(index))
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
    .run()
    .await
}
