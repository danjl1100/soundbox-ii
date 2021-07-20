use vlc_http::{self, Command, Controller, Credentials};

use std::io::{BufRead, Write};
use tokio::sync::mpsc::{channel, Sender};

fn prompt(tx: Sender<Command>) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut buffer = String::new();
    loop {
        print!("> ");
        stdout.flush()?;
        stdin.read_line(&mut buffer)?;
        let parts: Vec<&str> = buffer.trim().split(' ').collect();
        let cmd_str = parts[0];
        let cmd = match cmd_str {
            "" => {
                continue;
            }
            "quit" | "exit" => {
                break;
            }
            _ => parse_line(cmd_str, &parts[1..]),
        };
        if let Some(cmd) = cmd {
            tx.blocking_send(cmd).unwrap();
        } else {
            eprintln!("Unknown command: \"{}\"", cmd_str);
        }
        buffer.clear();
    }
    Ok(())
}
fn parse_line(cmd_str: &str, args: &[&str]) -> Option<Command> {
    match cmd_str {
        "play" => Some(Command::PlaybackResume),
        "pause" => Some(Command::PlaybackPause),
        "stop" => Some(Command::PlaybackStop),
        _ => None,
    }
}

#[actix_web::main]
async fn main() {
    println!("\nHello, soundbox-ii!\n");

    let host_port = Credentials::try_from_env().unwrap().unwrap();
    println!("Will connect to: {:?}", host_port);

    let (tx, rx) = channel(1);

    // spawn prompt
    std::thread::spawn(move || {
        prompt(tx).unwrap();
    });

    // run controller
    let controller = Controller::default();
    controller.run(host_port, rx).await;
}
