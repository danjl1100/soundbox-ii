use vlc_http::{self, Action, Command, PlaybackStatus, PlaylistInfo, Query, ResultReceiver};

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

pub struct Prompt {
    action_tx: mpsc::Sender<Action>,
    stdout: std::io::Stdout,
}
impl Prompt {
    pub(crate) fn new(action_tx: mpsc::Sender<Action>) -> Self {
        Self {
            action_tx,
            stdout: std::io::stdout(),
        }
    }
    pub(crate) fn run(&mut self) -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        let mut buffer = String::new();
        loop {
            // print prompt
            print!("> ");
            self.stdout.flush()?;
            // read line
            buffer.clear();
            let read_count = stdin.read_line(&mut buffer)?;
            if read_count == 0 {
                println!("<<STDIN EOF>>");
                break;
            }
            if !self.run_line(&buffer) {
                break;
            };
        }
        Ok(())
    }
    fn run_line(&mut self, line: &str) -> bool {
        // split args
        let parts: Vec<&str> = line.trim().split(' ').collect();
        let action_str = parts[0];
        match action_str {
            "" => {
                return true;
            }
            "quit" | "q" | "exit" => {
                return false;
            }
            _ => {}
        };
        // parse action
        let action_and_rx = parse_line(action_str, &parts[1..]);
        // execute action and print result
        match action_and_rx {
            Ok(ActionAndReceiver::Command(action, result_rx)) => {
                self.send_and_print_result(action, result_rx);
            }
            Ok(ActionAndReceiver::QueryPlaybackStatus(action, result_rx)) => {
                self.send_and_print_result(action, result_rx);
            }
            Ok(ActionAndReceiver::QueryPlaylistInfo(action, result_rx)) => {
                self.send_and_print_result(action, result_rx);
            }
            Err(message) => eprintln!("Input error: {}", message),
        }
        true
    }

    fn send_and_print_result<T>(&mut self, action: Action, result_rx: ResultReceiver<T>)
    where
        T: std::fmt::Debug,
    {
        // print action
        print!("running {} ", action);
        let print_a_dot = || {
            print!(".");
            drop(self.stdout.lock().flush());
        };
        print_a_dot();
        // send command
        #[allow(clippy::match_like_matches_macro)]
        let print_result = match &action {
            Action::Command(_, _) => false,
            _ => true,
        };
        self.action_tx.blocking_send(action).unwrap();
        // wait for result
        match blocking_recv(
            result_rx,
            std::time::Duration::from_millis(100),
            print_a_dot,
        ) {
            Some(Ok(action_result)) => {
                println!();
                if print_result {
                    dbg!(action_result);
                }
            }
            Some(Err(action_err)) => {
                dbg!(action_err);
            }
            None => println!("Failed to obtain command result"),
        }
    }
}

enum ActionAndReceiver {
    Command(Action, ResultReceiver<()>),
    QueryPlaybackStatus(Action, ResultReceiver<PlaybackStatus>),
    QueryPlaylistInfo(Action, ResultReceiver<PlaylistInfo>),
}
impl From<Command> for ActionAndReceiver {
    fn from(command: Command) -> Self {
        let (action, result_rx) = command.to_action_rx();
        Self::Command(action, result_rx)
    }
}
impl From<Query> for ActionAndReceiver {
    fn from(query: Query) -> Self {
        match query {
            Query::Art => todo!(),
            Query::PlaybackStatus => {
                let (action, result_rx) = Action::query_playback_status();
                Self::QueryPlaybackStatus(action, result_rx)
            }
            Query::PlaylistInfo => {
                let (action, result_rx) = Action::query_playlist_info();
                Self::QueryPlaylistInfo(action, result_rx)
            }
        }
    }
}
fn parse_line(action_str: &str, args: &[&str]) -> Result<ActionAndReceiver, String> {
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
    const QUERY_STATUS: &str = "status";
    const QUERY_PLAYLIST: &str = "playlist";
    let err_invalid_int = |_| "invalid integer number".to_string();
    let err_invalid_float = |_| "invalid decimal number".to_string();
    match action_str {
        CMD_PLAY => Ok(Command::PlaybackResume.into()),
        CMD_PAUSE => Ok(Command::PlaybackPause.into()),
        CMD_STOP => Ok(Command::PlaybackStop.into()),
        CMD_ADD => match args.split_first() {
            Some((uri, extra)) if extra.is_empty() => Ok(Command::PlaylistAdd {
                uri: uri.to_string(),
            }
            .into()),
            _ => Err("expected 1 argument (path/URI)".to_string()),
        },
        CMD_START => match args.split_first() {
            None => Ok(Command::PlaylistPlay { item_id: None }.into()),
            Some((item_id, extra)) if extra.is_empty() => Ok(Command::PlaylistPlay {
                item_id: Some(item_id.to_string()),
            }
            .into()),
            _ => Err("expected maximum of 1 argument (item id)".to_string()),
        },
        CMD_NEXT => Ok(Command::SeekNext.into()),
        CMD_PREV => Ok(Command::SeekPrevious.into()),
        CMD_SEEK => match args.split_first() {
            Some((seconds_str, extra)) if extra.is_empty() => seconds_str
                .parse()
                .map(|seconds| Command::SeekTo { seconds }.into())
                .map_err(err_invalid_int),
            _ => Err("expected 1 argument (seconds)".to_string()),
        },
        CMD_VOL => match args.split_first() {
            Some((percent_str, extra)) if extra.is_empty() => percent_str
                .parse()
                .map(|percent| Command::Volume { percent }.into())
                .map_err(err_invalid_int),
            _ => Err("expected 1 argument (percent)".to_string()),
        },
        CMD_SPEED => match args.split_first() {
            Some((speed_str, extra)) if extra.is_empty() => speed_str
                .parse()
                .map(|speed| Command::PlaybackSpeed { speed }.into())
                .map_err(err_invalid_float),
            _ => Err("expected 1 argument (decimal)".to_string()),
        },
        "." | QUERY_STATUS => Ok(Query::PlaybackStatus.into()),
        "p" | QUERY_PLAYLIST => Ok(Query::PlaylistInfo.into()),
        _ => Err(format!("Unknown command: \"{}\"", action_str)),
    }
}