use vlc_http::{self, Action, Command};

use std::convert::TryInto;
use std::io::{BufRead, Write};
use tokio::sync::{mpsc, oneshot};

fn blocking_recv<T, F: Fn()>(
    mut rx: oneshot::Receiver<T>,
    interval: std::time::Duration,
    wait_fn: F,
) -> Option<T> {
    loop {
        //std::thread::yield_now();
        std::thread::sleep(interval);
        match rx.try_recv() {
            Ok(t) => {
                return Some(t);
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(oneshot::error::TryRecvError::Closed) => {
                return None;
            }
        }
        wait_fn();
    }
}

fn prompt(action_tx: mpsc::Sender<Action>) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut buffer = String::new();
    loop {
        print!("> ");
        stdout.flush()?;
        stdin.read_line(&mut buffer)?;
        let parts: Vec<&str> = buffer.trim().split(' ').collect();
        let action_str = parts[0];
        let cmd = match action_str {
            "" => {
                continue;
            }
            "quit" | "exit" => {
                break;
            }
            _ => parse_line(action_str, &parts[1..]),
        };
        match cmd {
            Ok(cmd) => {
                // print action
                print!("running {:?} ", cmd);
                let print_a_dot = || {
                    print!(".");
                    drop(stdout.lock().flush());
                };
                print_a_dot();
                // send command
                let (action, result_rx) = cmd.to_action_rx();
                action_tx.blocking_send(action).unwrap();
                // wait for result
                match blocking_recv(
                    result_rx,
                    std::time::Duration::from_millis(100),
                    print_a_dot,
                ) {
                    Some(Ok(())) => {
                        println!();
                    }
                    Some(action_result) => {
                        let _ = dbg!(action_result);
                    }
                    None => println!("Failed to obtain command result"),
                }
            }
            Err(message) => eprintln!("Input error: {}", message),
        }
        buffer.clear();
    }
    Ok(())
}
fn parse_line(action_str: &str, args: &[&str]) -> Result<Command, String> {
    const CMD_PLAY: &str = "play";
    const CMD_PAUSE: &str = "pause";
    const CMD_STOP: &str = "stop";
    const CMD_ADD: &str = "add";
    const CMD_START: &str = "start";
    const CMD_NEXT: &str = "next";
    const CMD_PREV: &str = "prev";
    const CMD_SEEK: &str = "seek";
    const CMD_VOL: &str = "vol";
    const CMD_SPEED: &str = "speed";
    let err_invalid_int = |_| "invalid integer number".to_string();
    let err_invalid_float = |_| "invalid decimal number".to_string();
    match action_str {
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
                .map_err(err_invalid_int),
            _ => Err("expected 1 argument (seconds)".to_string()),
        },
        CMD_VOL => match args.split_first() {
            Some((percent_str, extra)) if extra.is_empty() => percent_str
                .parse()
                .map(|percent| Command::Volume { percent })
                .map_err(err_invalid_int),
            _ => Err("expected 1 argument (percent)".to_string()),
        },
        CMD_SPEED => match args.split_first() {
            Some((speed_str, extra)) if extra.is_empty() => speed_str
                .parse()
                .map(|speed| Command::PlaybackSpeed { speed })
                .map_err(err_invalid_float),
            _ => Err("expected 1 argument (decimal)".to_string()),
        },
        _ => Err(format!("Unknown command: \"{}\"", action_str)),
    }
}

#[tokio::main]
async fn main() {
    println!("\nHello, soundbox-ii!\n");

    let config = vlc_http::Config::try_from_env().expect("ENV vars set");
    let credentials = config.try_into().expect("valid host");
    println!("Will connect to: {:?}", credentials);

    let (action_tx, action_rx) = mpsc::channel(1);

    // spawn prompt
    std::thread::spawn(move || {
        prompt(action_tx).unwrap();
    });

    // run controller
    vlc_http::run(credentials, action_rx).await;
}
