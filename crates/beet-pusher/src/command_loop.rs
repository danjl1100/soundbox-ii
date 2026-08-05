// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Event loop logic to drive a [`BeetPusher`]

use std::sync::mpsc::RecvTimeoutError;

use eyre::Context as _;

use crate::{
    BeetCommand, BeetItem, BeetPusher, HintNeedPlaylistUpdate, Shutdown, fill_buckets,
    pipe_exec::{SpigotCmd, VlcCmd},
};

const TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
const FILL_PLAYLIST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
/// Returns the recommended timeout for clients listening for `beet_pusher` responses
#[must_use]
pub fn get_client_wait_timeout() -> std::time::Duration {
    const ADDED_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

    // NOTE: must be larger than `RESPONSE_TIMEOUT + FILL_PLAYLIST_INTERVAL`,
    // so add a suitable `ADDED_INTERVAL`
    RESPONSE_TIMEOUT + TICK_INTERVAL + ADDED_INTERVAL
}

/// Command loop to drive a [`BeetPusher`]
pub struct CommandLoop<'a, R, F> {
    loop_rx: std::sync::mpsc::Receiver<LoopEvent>,
    pusher: BeetPusher<'a, R>,
    http_runner: vlc_http_ureq::HttpRunner,
    now_playing_observer: F,
    beet_cmd: BeetCommand<'a>,
}
impl<'a, R, F, E> CommandLoop<'a, R, F>
where
    R: bucket_spigot::order::ArbitrarySource<Error: Send + Sync + 'static>,
    F: FnMut(&BeetItem) -> Result<(), E>,
    E: std::error::Error + Send + Sync + 'static,
{
    /// Creates a command loop with the specified resources
    pub fn new(
        pusher: BeetPusher<'a, R>,
        http_runner: vlc_http_ureq::HttpRunner,
        now_playing_observer: F,
        beet_cmd: BeetCommand<'a>,
    ) -> (Self, EventSender) {
        let (loop_tx, loop_rx) = std::sync::mpsc::sync_channel(1);

        let this = Self {
            loop_rx,
            pusher,
            http_runner,
            now_playing_observer,
            beet_cmd,
        };
        let sender = EventSender { loop_tx };

        (this, sender)
    }
    /// Runs the main command loop
    ///
    /// # Errors
    ///
    /// Returns an error if filling buckets fails
    // TODO: error behavior to be updated (related to issues/04-stdin-thread-dies-on-bad-line.md)
    ///
    /// # Panics
    ///
    /// Panics if the `loop_rx` ends without sending a shutdown message
    /// (guaranteed by [`EventSender`] drop impl)
    pub fn run(self) -> eyre::Result<()> {
        // TODO add a "determined holder" concept, to make it easy to:
        // 1. Peek a bunch, update spigot
        // 2. Load into VLC, retrieve "after current" items
        // 3. Pop from the "determined" holder
        // 4. Repeat from step 1, only peeking what is needed
        // ---> Prototype as a struct here, the move to bucket_spigot::order if it's generally useful

        let Self {
            loop_rx,
            mut pusher,
            mut http_runner,
            mut now_playing_observer,
            mut beet_cmd,
        } = self;

        let mut timer_fill_playlist = DebounceTask::new(FILL_PLAYLIST_INTERVAL);

        let mut timer_fill_bucket = DebounceTask::new(TICK_INTERVAL);
        // only fill buckets when explicitly activated
        timer_fill_bucket.set_active(false);

        loop {
            let event = if timer_fill_playlist.is_ready() {
                // highest priority - feed the externally-advancing VLC
                Ok(LoopEvent::MaintainPlaylist)
            } else if timer_fill_bucket.is_ready() {
                // only fill buckets when explicitly activated
                timer_fill_bucket.set_active(false);

                // next priority - update to reflect the user command
                Ok(LoopEvent::MaintainBuckets)
            } else {
                // lowest - process new commands
                loop_rx.recv_timeout(TICK_INTERVAL)
            };
            match event {
                Ok(loop_event) => match loop_event {
                    LoopEvent::Shutdown(Shutdown) => break,
                    LoopEvent::MaintainPlaylist => {
                        tracing::trace!("PLAYLIST UPDATE");
                        let hint_need_fill = pusher.push_playlist_update(
                            &mut http_runner,
                            Some(&mut now_playing_observer),
                        )?;
                        match hint_need_fill {
                            Some(HintNeedPlaylistUpdate::Immediate) => {
                                timer_fill_playlist.set_immediate();
                            }
                            Some(HintNeedPlaylistUpdate::WaitForVlc) => {
                                timer_fill_playlist
                                    .defer_with(std::time::Duration::from_millis(500));
                            }
                            None => {
                                // requires periodic maintenance (VLC client self-advances)
                                timer_fill_playlist.defer();
                            }
                        }
                    }
                    LoopEvent::MaintainBuckets => {
                        tracing::trace!("FILL BUCKETS");

                        let spigot = pusher.get_spigot_mut();
                        fill_buckets(&mut beet_cmd, spigot)?;

                        // NOTE: causes update to playlist, BUT cannot delay playlist upkeep
                        timer_fill_playlist.set_immediate();
                    }
                    LoopEvent::SpigotCmd { reply_to, cmd } => {
                        use crate::pipe_exec::{ErrorKind, ResponseData};
                        let spigot = pusher.get_spigot_mut();
                        tracing::trace!(?cmd);
                        let result = match spigot.modify_and_get_created_path(cmd.into()) {
                            Ok(Some(path)) => Ok(ResponseData::NodeAdded { path }),
                            Ok(None) => Ok(ResponseData::PassNoData),
                            Err(e) => Err(ErrorKind::SpigotError(e)),
                        };
                        let _ = reply_to.send(result);

                        // causes update to buckets
                        timer_fill_bucket.defer();
                        timer_fill_bucket.set_active(true);
                    }
                    LoopEvent::VlcCmd { reply_to, cmd } => {
                        use crate::pipe_exec::{ErrorKind, ResponseData};

                        match cmd {
                            VlcCmd::SeekNext => {
                                // causes update to playlist
                                timer_fill_playlist.set_immediate();
                            }
                        }

                        tracing::trace!(?cmd);
                        let result = match pusher.vlc_cmd(&mut http_runner, cmd) {
                            Ok(()) => Ok(ResponseData::PassNoData),
                            Err(e) => Err(ErrorKind::VlcRequest(e)),
                        };
                        let _ = reply_to.send(result);
                    }
                },
                Err(RecvTimeoutError::Disconnected) => {
                    #[expect(clippy::panic, reason = "invalid shutdown state")]
                    {
                        panic!("all senders ended with no Shutdown received")
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }

        Ok(())
    }
}

/// Accepts events to add to the command loop
pub struct EventSender {
    loop_tx: std::sync::mpsc::SyncSender<LoopEvent>,
}
impl EventSender {
    /// Documents the site of sending a shutdown request to the [`CommandLoop`], consuming this sender handle
    pub fn send_shutdown(self) {
        drop(self);
    }
}
impl Drop for EventSender {
    fn drop(&mut self) {
        let _ = self.loop_tx.send(Shutdown.into());
    }
}
impl AsRef<std::sync::mpsc::SyncSender<LoopEvent>> for EventSender {
    fn as_ref(&self) -> &std::sync::mpsc::SyncSender<LoopEvent> {
        &self.loop_tx
    }
}

/// Event runnable by the [`CommandLoop`] (sent via [`EventSender`])
enum LoopEvent {
    Shutdown(Shutdown),
    MaintainPlaylist,
    MaintainBuckets,
    SpigotCmd {
        reply_to: oneshot::Sender<crate::pipe_exec::ResponseResultInner>,
        cmd: SpigotCmd,
    },
    VlcCmd {
        reply_to: oneshot::Sender<crate::pipe_exec::ResponseResultInner>,
        cmd: VlcCmd,
    },
}
impl From<Shutdown> for LoopEvent {
    fn from(value: Shutdown) -> Self {
        Self::Shutdown(value)
    }
}

/// Helper to schedule the ready time for a task
struct DebounceTask {
    last_run: Option<std::time::Instant>,
    interval: std::time::Duration,
    is_active: bool,
}
impl DebounceTask {
    fn new(interval: std::time::Duration) -> Self {
        Self {
            last_run: None,
            interval,
            is_active: true,
        }
    }
    fn is_ready(&self) -> bool {
        let Self {
            last_run,
            interval,
            is_active,
        } = *self;

        if !is_active {
            return false;
        }

        if let Some(last_run) = last_run
            && last_run.elapsed() < interval
        {
            // last ran within the interval
            return false;
        }

        // ran too long ago, or never ran
        true
    }
    /// Schedules the task after the interval from now
    fn defer(&mut self) {
        self.last_run = Some(std::time::Instant::now());
    }
    /// Schedules the task after the specified interval (capped by [`Self::interval`]),
    /// overriding the built-in interval for one time only
    fn defer_with(&mut self, shorter_interval: std::time::Duration) {
        // Recall, the condition to run:  (SCHEDULED - last_run) >= interval
        //
        // Set fake `last_run` to force the shorter_interval
        //
        // <*> fake last_run
        //  |<------- interval ------->|
        //            |<--- shorter -->|
        //           <*> NOW          <*> SCHEDULED
        //
        //  |<------->|
        //       ^ subtract_from_now
        //
        let subtract_from_now = self
            .interval
            .checked_sub(shorter_interval)
            .unwrap_or_default();

        // fake `last_run` so that it will trigger after the custom interval
        self.last_run = std::time::Instant::now().checked_sub(subtract_from_now);
    }
    /// Blocks `is_ready` when `active` is false
    fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
    /// Sets the timer to ready when next active
    fn set_immediate(&mut self) {
        self.last_run = None;
    }
}

/// Pipes commands from stdin to the specified [`EventSender`]
///
/// # Errors
///
/// Returns an error if a stdin line is invalid
pub fn pipe_cmd_loop(loop_tx: &EventSender) -> eyre::Result<()> {
    use crate::pipe_exec::ResponseOut;

    for line in std::io::stdin().lines() {
        let line = line.context("failed to read from stdin")?;
        let result = ResponseOut::from(pipe_cmd(loop_tx, line));

        // view for json
        let result_json = serde_json::to_string(&result).context("failed to serialize error");

        if let ResponseOut::Error(err) = result {
            // consume to print eyre
            eprintln!("{:?}", eyre::eyre!(err));
        }

        // print JSON result (error if failed)
        println!("{}", result_json?);
    }

    Ok(())
}

/// Executes a line from stdin
///
/// # Errors
///
/// Returns an error if the [`LoopEvent`] callback fails
fn pipe_cmd(loop_tx: &EventSender, line: String) -> crate::pipe_exec::ResponseResult {
    use crate::pipe_exec::Command as PipeCommand;
    use crate::pipe_exec::{Error, ErrorKind};

    let crate::pipe_exec::CommandIn { seq, cmd } =
        serde_json::from_str(&line).map_err(|e| Error::new_invalid_command(line, e))?;

    let make_err = |kind| Error::new(seq, kind);

    let (reply_to, rx) = oneshot::channel();

    let event = match cmd {
        PipeCommand::Spigot(cmd) => LoopEvent::SpigotCmd { reply_to, cmd },
        PipeCommand::Vlc(cmd) => LoopEvent::VlcCmd { reply_to, cmd },
    };

    let _ = loop_tx.as_ref().send(event);
    match rx.recv_timeout(RESPONSE_TIMEOUT) {
        Ok(result) => result.map(|data| (seq, data)).map_err(make_err),
        Err(e) => match e {
            oneshot::RecvTimeoutError::Timeout | oneshot::RecvTimeoutError::Disconnected => {
                Err(make_err(ErrorKind::InternalTimeout))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{RESPONSE_TIMEOUT, TICK_INTERVAL, get_client_wait_timeout};

    #[test]
    fn client_wait_timeout_invariants() {
        let uut = get_client_wait_timeout();
        assert!(
            uut > RESPONSE_TIMEOUT + TICK_INTERVAL,
            "client wait timeout is too short: {uut:?}"
        );
    }
}
