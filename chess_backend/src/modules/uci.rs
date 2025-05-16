use std::io::{BufRead, BufReader, Write, BufWriter};
use std::process::{Command, Stdio, Child, ChildStdout, ChildStdin};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use actix::prelude::*;
use actix_web_actors::ws::{WebsocketContext, Message, ProtocolError};

// Minimal MyWebSocketActor definition
pub struct MyWebSocketActor;
impl MyWebSocketActor {
    pub fn new() -> Self {
        MyWebSocketActor
    }
}
impl actix::Actor for MyWebSocketActor {
    type Context = WebsocketContext<Self>;
}

impl actix::StreamHandler<Result<Message, ProtocolError>> for MyWebSocketActor {
    fn handle(&mut self, msg: Result<Message, ProtocolError>, ctx: &mut WebsocketContext<Self>) {
        match msg {
            Ok(Message::Ping(msg)) => ctx.pong(&msg),
            Ok(Message::Text(text)) => ctx.text(format!("Echo: {}", text)),
            Ok(Message::Binary(bin)) => ctx.binary(bin),
            Ok(Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            },
            Ok(Message::Continuation(_)) => {},
            Ok(Message::Pong(_)) => {},
            Ok(Message::Nop) => {},
            Err(_) => ctx.stop(),
        }
    }
}

pub struct UciEngine {
    process: Child,
    input: BufWriter<ChildStdin>,
    output: Arc<Mutex<BufReader<ChildStdout>>>,
    address: Option<Addr<MyWebSocketActor>>,
}

impl UciEngine {
    pub fn start() -> Result<Self, std::io::Error> {
        let mut process = Command::new("target/debug/chess_engine")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Take ownership of the handles
        let input = BufWriter::new(process.stdin.take().unwrap());
        let output = Arc::new(Mutex::new(BufReader::new(process.stdout.take().unwrap())));
        Ok(Self {
            process,
            input,
            output,
            address: None,
        })
    }

    pub fn set_address(&mut self, address: Addr<MyWebSocketActor>) {
        self.address = Some(address);
    }

    pub fn send_command(&mut self, command: &str) -> Result<(), std::io::Error> {
        println!("> [SEND] {}", command);
        writeln!(&mut self.input, "{}", command)?;
        self.input.flush()?;
        Ok(())
    }

    pub fn read_line(&self) -> Result<String, std::io::Error> {
        let mut output_guard = self.output.lock().unwrap();
        let mut buffer = String::new();
        output_guard.read_line(&mut buffer)?;
        buffer = buffer.trim().to_string();
        println!("< [RECV] {}", buffer);
        Ok(buffer)
    }

    pub fn read_until(&self, target: &str) -> Result<String, std::io::Error> {
        let mut output_guard = self.output.lock().unwrap();
        let mut buffer = String::new();
        let mut result = String::new();
        loop {
            buffer.clear();
            output_guard.read_line(&mut buffer)?;
            result.push_str(&buffer);
            let trimmed_buffer = buffer.trim();
            println!("< [RECV] {}", trimmed_buffer);
            if trimmed_buffer.contains(target) {
                break;
            }
        }
        Ok(result)
    }
}

impl Actor for UciEngine {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        // Async logic can be added here if needed
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        println!("Killing UCI engine...");
        self.process.kill().unwrap();
    }
}

impl UciEngine {
    pub async fn start_uci_engine(address: Addr<MyWebSocketActor>) {
        let mut engine = UciEngine::start().unwrap();
        engine.set_address(address);
        engine.send_command("ucinewgame").unwrap();
        engine.send_command("position startpos").unwrap();
        engine.send_command("go depth 2").unwrap();
        let best_move = engine.read_until("bestmove").unwrap();
        println!("best move is {}", best_move);

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            engine.send_command("isready").unwrap();
            engine.read_until("readyok").unwrap();
        }
    }
}
