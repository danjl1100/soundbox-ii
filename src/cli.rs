use shared::Shutdown;
use vlc_http::{self, Action, Command, PlaybackStatus, PlaylistInfo, Query, ResultReceiver};

use std::io::{BufRead, Write};
use tokio::sync::{mpsc, oneshot, watch};

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

pub struct Config {
    pub action_tx: mpsc::Sender<Action>,
    pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
    pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
}
pub struct Prompt {
    action_tx: mpsc::Sender<Action>,
    playback_status: SyncWatchReceiver<Option<PlaybackStatus>>,
    playlist_info: SyncWatchReceiver<Option<PlaylistInfo>>,
    stdout: std::io::Stdout,
}
impl Config {
    pub(crate) fn build(self) -> Prompt {
        let Self {
            action_tx,
            playback_status_rx,
            playlist_info_rx,
        } = self;
        Prompt {
            action_tx,
            playback_status: SyncWatchReceiver::new(playback_status_rx),
            playlist_info: SyncWatchReceiver::new(playlist_info_rx),
            stdout: std::io::stdout(),
        }
    }
}
impl Prompt {
    pub(crate) fn run_until<F>(mut self, is_shutdown_fn: F) -> std::io::Result<()>
    where
        F: Fn() -> Option<Shutdown>,
    {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        let mut buffer = String::new();
        loop {
            if let Some(Shutdown) = is_shutdown_fn() {
                break;
            }
            // print prompt
            print!("soundbox-ii> ");
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
        let parsed = match action_str {
            "" => {
                // skip action, just poll and print status
                None
            }
            "quit" | "q" | "exit" => {
                println!("exit\n");
                return false;
            }
            _ => {
                // parse action
                Some(parse_line(action_str, &parts[1..]))
            }
        };
        if let Some(action_and_rx) = parsed {
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
        }
        // poll and print status
        if let Some(Some(playback)) = self.playback_status.poll_update() {
            dbg!(playback);
        }
        if let Some(Some(playlist)) = self.playlist_info.poll_update() {
            dbg!(playlist);
        }

        true
    }

    fn send_and_print_result<T>(
        &mut self,
        action: Action,
        result_rx: ResultReceiver<T>,
    ) -> Option<T>
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
                    dbg!(&action_result);
                }
                Some(action_result)
            }
            Some(Err(action_err)) => {
                dbg!(action_err);
                None
            }
            None => {
                println!("Failed to obtain command result");
                None
            }
        }
    }
}
struct SyncWatchReceiver<T>
where
    T: PartialEq + Clone,
{
    receiver: watch::Receiver<T>,
    prev_value: Option<T>,
}
impl<T> SyncWatchReceiver<T>
where
    T: PartialEq + Clone + std::fmt::Debug,
{
    fn new(receiver: watch::Receiver<T>) -> Self {
        Self {
            receiver,
            prev_value: None,
        }
    }
    fn poll_update(&mut self) -> Option<&T> {
        //TODO: too many `clone`s!
        let current = self.receiver.borrow().clone();
        self.update_changed(current)
    }
    fn update_changed(&mut self, value: T) -> Option<&T> {
        // detect change in value
        let identical = matches!(&self.prev_value, Some(prev) if *prev == value);
        let changed = !identical;
        // update regardless (no harm if same)
        self.prev_value = Some(value);
        // if changed, give updated value ref
        if changed {
            self.prev_value.as_ref()
        } else {
            None
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
        use vlc_http::IntoAction;
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
