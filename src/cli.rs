// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::seq::{self, SequencerAction};
use arg_split::ArgSplit;
use shared::Shutdown;
use std::io::{BufRead, Write};
use tokio::sync::{mpsc, oneshot, watch};
use vlc_http::{self, PlaybackStatus, PlaylistInfo};

pub use command::{parse_url, COMMAND_NAME};
/// Definition of all interactive commands
pub(crate) mod command;

fn blocking_recv<T>(
    mut rx: oneshot::Receiver<T>,
    interval: std::time::Duration,
    mut wait_fn: impl FnMut(),
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
pub(crate) struct Config {
    pub vlc_tx: mpsc::Sender<vlc_http::Action>,
    pub sequencer_tx: mpsc::Sender<seq::SequencerAction>,
    pub sequencer_state_rx: watch::Receiver<String>,
    pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
    pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
}
pub struct Prompt {
    vlc_tx: mpsc::Sender<vlc_http::Action>,
    sequencer_tx: mpsc::Sender<seq::SequencerAction>,
    sequencer_state: SyncWatchReceiver<String>,
    playback_status: SyncWatchReceiver<Option<PlaybackStatus>>,
    playlist_info: SyncWatchReceiver<Option<PlaylistInfo>>,
    stdout: std::io::Stdout,
}
impl Config {
    pub(crate) fn build(self) -> Prompt {
        let Self {
            vlc_tx,
            sequencer_tx,
            sequencer_state_rx,
            playback_status_rx,
            playlist_info_rx,
        } = self;
        Prompt {
            vlc_tx,
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

        // execute command
        if let Some(command) = parsed.command {
            // execute action and print result
            match command.try_build()? {
                Err(shutdown_option) => return Ok(shutdown_option),
                Ok(result_and_rx) => result_and_rx.exec(self),
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

    fn print_a_dot_fn(stdout: &mut std::io::Stdout) -> impl FnMut() + '_ {
        || {
            eprint!(".");
            drop(stdout.lock().flush());
        }
    }
    fn sender_vlc(&mut self) -> SenderDotSession<'_, vlc_http::Action, impl FnMut() + '_> {
        const TYPE_VLC: &str = "vlc";
        SenderDotSession {
            sender: &mut self.vlc_tx,
            print_a_dot: Self::print_a_dot_fn(&mut self.stdout),
            ty: TYPE_VLC,
        }
    }
    fn sender_seq(&mut self) -> SenderDotSession<'_, SequencerAction, impl FnMut() + '_> {
        const TYPE_SEQ: &str = "sequencer";
        SenderDotSession {
            sender: &mut self.sequencer_tx,
            print_a_dot: Self::print_a_dot_fn(&mut self.stdout),
            ty: TYPE_SEQ,
        }
    }
}
struct SenderDotSession<'a, T, U> {
    sender: &'a mut mpsc::Sender<T>,
    print_a_dot: U,
    ty: &'static str,
}
impl<'a, T, U> SenderDotSession<'a, T, U> {
    const WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
    fn send_and_print_result<V, E>(
        self,
        action: T,
        result_rx: oneshot::Receiver<Result<V, E>>,
    ) -> Result<(), String>
    where
        T: std::fmt::Display,
        U: FnMut(),
        V: std::fmt::Debug,
        E: std::fmt::Display,
    {
        let Self {
            sender,
            mut print_a_dot,
            ty,
        } = self;
        // print action
        eprint!("running {ty} {action} ");
        print_a_dot();
        // send command
        sender
            .blocking_send(action)
            .map_err(|_| "Failed to send {ty}-command action".to_string())?;
        // wait for result
        match blocking_recv(result_rx, Self::WAIT_INTERVAL, print_a_dot) {
            Some(Ok(action_result)) => {
                eprintln!(); // clear dot-line

                // TODO is `RESULT` useful for the cli user?
                println!("RESULT: {action_result:?}");
                Ok(())
            }
            Some(Err(action_err)) => Err(format!("{ty}-command returned error: {action_err}")),
            None => Err("Failed to obtain {ty}-command result".to_string()),
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
