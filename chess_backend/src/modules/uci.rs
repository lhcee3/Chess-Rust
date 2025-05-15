use std::io::{BufRead, BufReader, Write, BufWriter, Stdout};
    use std::process::{Command, Stdio, Child, ChildStdout};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use actix::prelude::*;
    use actix_web::rt::spawn;

    pub struct UciEngine {
        process: Child,
        input: BufWriter<std::process::ChildStdin>,
        output: Arc<Mutex<BufReader<ChildStdout>>>,
        address: Addr<super::MyWebSocketActor>,
    }

    impl UciEngine {
        pub fn start() -> Result<Self, std::io::Error> {
            let process = Command::new("target/debug/chess_engine")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;

            let input = BufWriter::new(process.stdin.as_ref().unwrap().try_clone().unwrap());
            let output = Arc::new(Mutex::new(BufReader::new(process.stdout.as_ref().unwrap().try_clone().unwrap())));
            Ok(Self {
                process,
                input,
                output,
                address: Addr::default(),
            })
        }

        pub fn set_address(&mut self, address: Addr<super::MyWebSocketActor>) {
            self.address = address;
        }

        pub fn send_command(&mut self, command: &str) -> Result<(), std::io::Error> {
            println!("> [SEND] {}", command); // Added [SEND] tag
            writeln!(&mut self.input, "{}", command)?;
            self.input.flush()?;
            Ok(())
        }

        pub fn read_line(&self) -> Result<String, std::io::Error> {
            let mut output_guard = self.output.lock().unwrap();
            let mut buffer = String::new();
            output_guard.read_line(&mut buffer)?;
            buffer = buffer.trim().to_string();
            println!("< [RECV] {}", buffer); // Added [RECV] tag
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
                println!("< [RECV] {}", trimmed_buffer); // Added [RECV] tag
                if trimmed_buffer.contains(target) {
                    break;
                }
            }
            Ok(result)
        }
    }

    impl Actor for UciEngine {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            let mut engine = self;
            spawn(async move {
                engine.send_command("uci").unwrap();
                let uci_ok = engine.read_until("uciok").unwrap();
                assert!(uci_ok.contains("uciok"), "Expected 'uciok' in response to 'uci'");

                engine.send_command("isready").unwrap();
                let ready_ok = engine.read_until("readyok").unwrap();
                assert!(ready_ok.contains("readyok"), "Expected 'readyok' in response to 'isready'");
            });
        }

        fn stopped(&mut self, _ctx: &mut Self::Context) {
            println!("Killing UCI engine...");
            self.process.kill().unwrap();
        }
    }

    impl UciEngine {
        pub async fn start_uci_engine(address: Addr<super::MyWebSocketActor>) {
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
