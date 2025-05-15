use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web::middleware::Logger;
use actix_files as fs;
use chess_backend::add;
use actix_web::rt::spawn;
use actix_web::web::Payload;
use actix_web::{HttpRequest, Error};
use actix_web_actors::ws;
use crate::uci::start_uci_engine;
use std::time::Duration;

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

struct MyWebSocketActor {
}

impl MyWebSocketActor {
    fn new() -> Self {
        MyWebSocketActor {}
    }
}

use actix::prelude::*;
use actix::StreamHandler;

impl Actor for MyWebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        spawn(async move {
            start_uci_engine(addr).await;
        });
        ctx.text("connected");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                println!("Received text: {}", text);
                ctx.text(format!("Echo: {}", text));
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                println!("Connection closed with reason: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => println!("Continuation"),
            Ok(ws::Message::Pong(_)) => println!("Pong"),
            Err(e) => {
                eprintln!("Error: {:?}", e);
                ctx.stop();
            }
        }
    }
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