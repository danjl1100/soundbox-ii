// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::seq;
use arg_util::ArgSplit;
use shared::Shutdown;
use std::io::{BufRead, Write};
use tokio::sync::{mpsc, watch};
use vlc_http::{PlaybackStatus, PlaylistInfo};

pub use command::{parse_url, COMMAND_NAME};
/// Definition of interactive commands
pub(crate) mod command;

pub(crate) struct Config {
    pub vlc_tx: mpsc::Sender<vlc_http::Action>,
    pub sequencer_tx: mpsc::Sender<seq::SequencerAction>,
    pub sequencer_cli_tx: mpsc::Sender<seq::NodeCommand>,
    pub sequencer_state_rx: watch::Receiver<SequencerState>,
    pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
    pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
}
pub struct Prompt {
    vlc_tx: mpsc::Sender<vlc_http::Action>,
    sequencer_tx: mpsc::Sender<seq::SequencerAction>,
    sequencer_cli_tx: mpsc::Sender<seq::NodeCommand>,
    sequencer_state: SyncWatchReceiver<SequencerState>,
    playback_status: SyncWatchReceiver<Option<PlaybackStatus>>,
    playlist_info: SyncWatchReceiver<Option<PlaylistInfo>>,
}
impl Config {
    pub(crate) fn build(self) -> Prompt {
        let Self {
            vlc_tx,
            sequencer_tx,
            sequencer_cli_tx,
            sequencer_state_rx,
            playback_status_rx,
            playlist_info_rx,
        } = self;
        Prompt {
            vlc_tx,
            sequencer_tx,
            sequencer_cli_tx,
            sequencer_state: SyncWatchReceiver::new(sequencer_state_rx),
            playback_status: SyncWatchReceiver::new(playback_status_rx),
            playlist_info: SyncWatchReceiver::new(playlist_info_rx),
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
            eprint_flush(format_args!("{} ", command::PROMPT_STR));
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
                    eprintln!("{clap_err}");
                }
                Err(Error::Message(e)) => {
                    eprintln!("ERROR: {e}");
                }
            }
        }
        Ok(())
    }
}
fn eprint_flush(fmt_args: std::fmt::Arguments) {
    eprint!("{fmt_args}");
    std::io::stderr().flush().expect("stderr flush");
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
        let line_parts = ArgSplit::split_into_owned(line);
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
            println!("Playback: {playback:#?}");
        }
        if let Some(Some(playlist)) = self.playlist_info.poll_update() {
            println!("Playlist: {playlist:#?}");
        }
        if let Some(sequencer_state) = self.sequencer_state.poll_update() {
            println!("Sequencer state: {sequencer_state}");
        }

        Ok(None)
    }
}

mod sender_dot_session {
    use super::{eprint_flush, Prompt};
    use crate::seq::{self, SequencerAction};
    use tokio::sync::{mpsc, oneshot};

    pub(super) struct SenderDotSession<'a, T> {
        sender: &'a mut mpsc::Sender<T>,
        ty: &'static str,
    }
    impl Prompt {
        pub(super) fn sender_vlc(&mut self) -> SenderDotSession<'_, vlc_http::Action> {
            const TYPE_VLC: &str = "vlc";
            SenderDotSession {
                sender: &mut self.vlc_tx,
                ty: TYPE_VLC,
            }
        }
        pub(super) fn sender_seq(&mut self) -> SenderDotSession<'_, SequencerAction> {
            const TYPE_SEQ: &str = "sequencer";
            SenderDotSession {
                sender: &mut self.sequencer_tx,
                ty: TYPE_SEQ,
            }
        }
        pub(super) fn sender_seq_cli(&mut self) -> SenderDotSession<'_, seq::NodeCommand> {
            const TYPE_SEQ: &str = "sequencer-cli";
            SenderDotSession {
                sender: &mut self.sequencer_cli_tx,
                ty: TYPE_SEQ,
            }
        }
    }
    impl<'a, T> SenderDotSession<'a, T>
    where
        T: std::fmt::Display,
    {
        const WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
        pub(super) fn send_and_print_result<V, E>(
            mut self,
            action: T,
            result_rx: oneshot::Receiver<Result<V, E>>,
        ) -> Result<(), String>
        where
            V: std::fmt::Debug,
            E: std::fmt::Display,
        {
            self.send(action)?;
            let action_result = self.wait_and_print_result(result_rx)?;

            // TODO is `RESULT` useful for the cli user?
            println!("RESULT: {action_result:?}");

            Ok(())
        }
        fn wait_and_print_result<V, E>(
            self,
            result_rx: oneshot::Receiver<Result<V, E>>,
        ) -> Result<V, String>
        where
            V: std::fmt::Debug,
            E: std::fmt::Display,
        {
            let Self { ty, .. } = self;
            let print_a_dot_fn = || {
                eprint_flush(format_args!("."));
            };
            print_a_dot_fn();
            // wait for result
            match blocking_recv(result_rx, Self::WAIT_INTERVAL, print_a_dot_fn) {
                Some(Ok(action_result)) => {
                    eprintln!(); // clear dot-line
                    Ok(action_result)
                }
                Some(Err(action_err)) => Err(format!("{ty}-command returned error: {action_err}")),
                None => Err("Failed to obtain {ty}-command result".to_string()),
            }
        }
        pub(super) fn send(&mut self, action: T) -> Result<(), String> {
            let Self { sender, ty, .. } = self;
            // print action
            eprint!("running {ty} {action} ");
            // send command
            sender
                .blocking_send(action)
                .map_err(|_| "Failed to send {ty}-command action".to_string())
        }
    }

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
        let current = self.receiver.borrow();
        Self::update_changed(&mut self.prev_value, &current)
    }
    fn update_changed<'a>(prev_value: &'a mut Option<T>, current_value: &T) -> Option<&'a T> {
        // detect change in value
        match prev_value {
            Some(prev) if prev == current_value => None,
            _ => {
                // changed, give updated value ref
                prev_value.replace(current_value.clone());
                prev_value.as_ref()
            }
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerState(pub String);
impl std::fmt::Display for SequencerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
