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
        match cmd {
            Ok(cmd) => tx.blocking_send(cmd).unwrap(),
            Err(message) => eprintln!("Input error: {}", message),
        }
        buffer.clear();
    }
    Ok(())
}
fn parse_line(cmd_str: &str, args: &[&str]) -> Result<Command, String> {
    const CMD_PLAY: &str = "play";
    const CMD_PAUSE: &str = "pause";
    const CMD_STOP: &str = "stop";
    const CMD_ADD: &str = "add";
    const CMD_START: &str = "start";
    const CMD_NEXT: &str = "next";
    const CMD_PREV: &str = "prev";
    const CMD_SEEK: &str = "seek";
    match cmd_str {
        CMD_PLAY => Ok(Command::PlaybackResume),
        CMD_PAUSE => Ok(Command::PlaybackPause),
        CMD_STOP => Ok(Command::PlaybackStop),
        CMD_ADD => match args.split_first() {
            Some((uri, extra)) if extra.is_empty() => Ok(Command::PlaylistAdd {
                uri: uri.to_string(),
            }),
            _ => Err("expected 1 argument (path/URI)".to_string()),
        },
        CMD_START => match args.split_first() {
            None => Ok(Command::PlaylistPlay { item_id: None }),
            Some((item_id, extra)) if extra.is_empty() => Ok(Command::PlaylistPlay {
                item_id: Some(item_id.to_string()),
            }),
            _ => Err("expected maximum of 1 argument (item id)".to_string()),
        },
        CMD_NEXT => Ok(Command::SeekNext),
        CMD_PREV => Ok(Command::SeekPrevious),
        CMD_SEEK => match args.split_first() {
            Some((seconds_str, extra)) if extra.is_empty() => seconds_str
                .parse()
                .map(|seconds| Command::SeekTo { seconds })
                .map_err(|_| "invalid number".to_string()),
            _ => Err("expected 1 argument (seconds)".to_string()),
        },
        _ => Err(format!("Unknown command: \"{}\"", cmd_str)),
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
