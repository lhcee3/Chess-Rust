use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web::middleware::Logger;
use actix_web::{HttpRequest, Error};
use actix_web::web::Payload;
use actix_web_actors::ws;
use chess_backend::modules::uci::MyWebSocketActor;
use env_logger;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Message {
    text: String,
}

async fn ws_handler(
    req: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, Error> {
    ws::start(MyWebSocketActor::new(), &req, payload)
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Chess Backend with Actix Web and WebSockets")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(web::resource("/").to(index))
            .route("/ws/", web::get().to(ws_handler))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}