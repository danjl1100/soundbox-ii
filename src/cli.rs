// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::seq;
use arg_split::ArgSplit;
use shared::Shutdown;
use std::io::{BufRead, Write};
use tokio::sync::{mpsc, oneshot, watch};
use vlc_http::{self, PlaybackStatus, PlaylistInfo, ResultReceiver};

use command::ActionAndReceiver;
pub use command::{parse_url, COMMAND_NAME};
/// Definition of all interactive commands
pub(crate) mod command;

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
    pub action_tx: mpsc::Sender<vlc_http::Action>,
    pub sequencer_tx: mpsc::Sender<seq::SequencerCommand>,
    pub sequencer_state_rx: watch::Receiver<String>,
    pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
    pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
}
pub struct Prompt {
    action_tx: mpsc::Sender<vlc_http::Action>,
    sequencer_tx: mpsc::Sender<seq::SequencerCommand>,
    sequencer_state: SyncWatchReceiver<String>,
    playback_status: SyncWatchReceiver<Option<PlaybackStatus>>,
    playlist_info: SyncWatchReceiver<Option<PlaylistInfo>>,
    stdout: std::io::Stdout,
}
impl Config {
    pub(crate) fn build(self) -> Prompt {
        let Self {
            action_tx,
            sequencer_tx,
            sequencer_state_rx,
            playback_status_rx,
            playlist_info_rx,
        } = self;
        Prompt {
            action_tx,
            sequencer_tx,
            sequencer_state: SyncWatchReceiver::new(sequencer_state_rx),
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
            eprint!("{} ", command::PROMPT_STR);
            self.stdout.flush()?;
            // read line
            buffer.clear();
            let read_count = stdin.read_line(&mut buffer)?;
            if read_count == 0 {
                eprintln!("<<STDIN EOF>>");
                break;
            }
            match self.run_line(&buffer) {
                Ok(Some(Shutdown)) => {
                    eprintln!("exit");
                    break;
                }
                Ok(None) => {}
                Err(Error::Clap(clap_err)) => {
                    eprintln!("{}", clap_err);
                }
                Err(Error::Message(e)) => {
                    eprintln!("ERROR: {}", e);
                }
            };
        }
        Ok(())
    }
}

shared::wrapper_enum! {
    enum Error {
        Clap(clap::Error),
        Message(String),
    }
}

impl Prompt {
    fn run_line(&mut self, line: &str) -> Result<Option<Shutdown>, Error> {
        use clap::Parser;
        // split args - allow quoted strings with whitespace, and allow escape characters (`\"`) etc
        let line_parts = ArgSplit::split(line);
        let parsed = command::InteractiveArgs::try_parse_from(line_parts)?;
        if let Some(command) = parsed.command {
            // execute action and print result
            match command.try_build()? {
                Err(shutdown_option) => return Ok(shutdown_option),
                Ok(result_and_rx) => match result_and_rx {
                    ActionAndReceiver::VlcCommand(action, result_rx) => {
                        self.send_and_print_result(action, result_rx)
                    }
                    ActionAndReceiver::SequencerCommand(command) => self
                        .sequencer_tx
                        .blocking_send(command)
                        .map_err(|_| "Failed to send sequencer-command result".to_string()),
                    ActionAndReceiver::VlcQueryPlayback(action, result_rx) => {
                        self.send_and_print_result(action, result_rx)
                    }
                    ActionAndReceiver::VlcQueryPlaylist(action, result_rx) => {
                        self.send_and_print_result(action, result_rx)
                    }
                },
            }?;
        }
        //TODO: if there is a command error, these status print-outs can obscure the error
        // --> need to defer these on error (show a symbol for "press enter to see update")
        //      OR find a way to print the error *below* these status updates,
        //      possibly re-printing the user input for clarity

        // poll and print status
        if let Some(Some(playback)) = self.playback_status.poll_update() {
            println!("Playback: {:#?}", playback);
        }
        if let Some(Some(playlist)) = self.playlist_info.poll_update() {
            println!("Playlist: {:#?}", playlist);
        }
        if let Some(sequencer_state) = self.sequencer_state.poll_update() {
            println!("Sequencer state: {}", sequencer_state);
        }

        Ok(None)
    }

    fn send_and_print_result<T>(
        &mut self,
        action: vlc_http::Action,
        result_rx: ResultReceiver<T>,
    ) -> Result<(), String>
    where
        T: std::fmt::Debug,
    {
        // print action
        eprint!("running vlc {action} ");
        let print_a_dot = || {
            eprint!(".");
            drop(self.stdout.lock().flush());
        };
        print_a_dot();
        // send command
        let send_result = self.action_tx.blocking_send(action);
        if send_result.is_err() {
            return Err("Failed to send vlc-command result".to_string());
        }
        // wait for result
        match blocking_recv(
            result_rx,
            std::time::Duration::from_millis(100),
            print_a_dot,
        ) {
            Some(Ok(_action_result)) => {
                eprintln!();
                Ok(())
            }
            Some(Err(action_err)) => {
                dbg!(action_err);
                Err("vlc-action returned error".to_string())
            }
            None => Err("Failed to obtain command result".to_string()),
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
        //  maybe use `has_changed` to determine when to clone?
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
