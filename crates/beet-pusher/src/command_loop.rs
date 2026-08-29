// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Event loop logic to drive a [`BeetPusher`]

use crossbeam_channel::RecvTimeoutError;

use eyre::Context as _;
use vlc_http_ureq::HttpRunner;

use crate::{
    BeetCommand, BeetItem, BeetPusher, HintNeedPlaylistUpdate, Shutdown,
    beet::{apply_bucket_fill_results, get_bucket_fill_needs},
    pipe_exec::{SpigotCmd, VlcCmd},
    pusher::{Error as PusherError, VlcDriver},
};

mod bucket_fill;
mod queue;
mod vlc_act;

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
    loop_rx: crossbeam_channel::Receiver<LoopEvent>,
    loop_tx: crossbeam_channel::Sender<LoopEvent>,
    pusher: BeetPusher<'a, R>,
    http_runner: vlc_http_ureq::HttpRunner,
    now_playing_observer: F,
    beet_cmd: BeetCommand<'static>,
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
        beet_cmd: BeetCommand<'static>,
    ) -> (Self, EventSender) {
        let (loop_tx, loop_rx) = crossbeam_channel::bounded(1);

        let this = Self {
            loop_rx,
            loop_tx: loop_tx.clone(),
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
    #[expect(
        clippy::too_many_lines,
        reason = "TODO refactor branches into struct methods"
    )]
    pub fn run(self) -> Result<(), LoopError<E, R::Error>> {
        // TODO add a "determined holder" concept, to make it easy to:
        // 1. Peek a bunch, update spigot
        // 2. Load into VLC, retrieve "after current" items
        // 3. Pop from the "determined" holder
        // 4. Repeat from step 1, only peeking what is needed
        // ---> Prototype as a struct here, the move to bucket_spigot::order if it's generally useful

        let Self {
            loop_rx,
            loop_tx,
            mut pusher,
            http_runner,
            mut now_playing_observer,
            beet_cmd,
        } = self;

        let mut vlc_exponential_backoff: Option<ExponentialBackoff> = None;

        let (bucket_fill_tx, bucket_fill_thread) = bucket_fill::spawn(loop_tx.clone(), beet_cmd);

        let (vlc_act_tx, vlc_act_thread) = {
            let vlc_driver = VlcDriver::default_from_runner(http_runner);
            vlc_act::spawn(loop_tx, vlc_driver)
        };

        let loop_result: Result<(), LoopErrorKind<E, R::Error>> = (|| {
            let mut timer_fill_playlist = DebounceTask::new(FILL_PLAYLIST_INTERVAL);

            let mut timer_fill_bucket = DebounceTask::new(TICK_INTERVAL);
            // only fill buckets when explicitly activated
            timer_fill_bucket.set_active(false);

            let mut shutdown_active = false;

            loop {
                let event_result = if let Ok(loop_event) = loop_rx.try_recv() {
                    // highest priority - maintenance requires regular processing of events
                    // for multi-step (background) processes
                    Ok(loop_event)
                    // NOTE: Ignoring `Err(_)` case, as this should be replayed in the last `else` below
                } else if timer_fill_playlist.take_ready() {
                    // next priority - feed the externally-advancing VLC
                    Ok(LoopEvent::MaintainPlaylistStart)
                } else if timer_fill_bucket.take_ready() {
                    // lowest priority - update to reflect user commands
                    Ok(LoopEvent::BucketFillStart)
                } else if shutdown_active && timer_fill_playlist.is_active {
                    break;
                } else {
                    // lowest - wait to process new commands (same as first case, but with interval)
                    loop_rx.recv_timeout(TICK_INTERVAL)
                };

                let loop_event = match event_result {
                    Ok(loop_event) => loop_event,
                    Err(RecvTimeoutError::Disconnected) => {
                        #[expect(clippy::panic, reason = "invalid shutdown state")]
                        {
                            panic!("all senders ended with no Shutdown received")
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                };

                tracing::trace!(?loop_event);

                match loop_event {
                    LoopEvent::Shutdown(Shutdown) => shutdown_active = true,
                    LoopEvent::MaintainPlaylistStart => {
                        let mut queued_vlc_act = false;

                        if let Some(action) = pusher
                            .get_playlist_update_action()
                            .map_err(LoopErrorKind::PullItems)?
                        {
                            let result = vlc_act_tx.send_playlist_update_action(action);
                            queued_vlc_act = result.is_ok();
                            warn_queue_failed(result, "VLC playlist maintenance action");
                        }

                        if !queued_vlc_act {
                            // resume timer, in case playlist needs change
                            timer_fill_playlist.defer();
                            timer_fill_playlist.set_active(true);
                        }
                    }
                    LoopEvent::MaintainPlaylistResponse(result) => {
                        // resume timer, no matter the result
                        timer_fill_playlist.set_active(true);

                        let vlc_act::PlaylistResponse(playlist_update_counts) = result;

                        let result = pusher.push_playlist_update_action(
                            playlist_update_counts,
                            Some(&mut now_playing_observer),
                        );
                        let timer_hint_result: Result<
                            Option<HintNeedPlaylistUpdate>,
                            HintRetryableError,
                        > = match result {
                            Ok(hint) => Ok(Ok(hint)),
                            Err(err) => {
                                if let Some(retryable) = is_retryable(&err) {
                                    let err = eyre::eyre!(err);
                                    eprintln!("{err:?}");
                                    Ok(Err(retryable))
                                } else {
                                    Err(LoopErrorKind::PlaylistUpdate(err))
                                }
                            }
                        }?;

                        tracing::trace!(?timer_hint_result);

                        if let Ok(hint) = timer_hint_result {
                            vlc_exponential_backoff.take();

                            match hint {
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
                        } else {
                            let backoff = vlc_exponential_backoff.get_or_insert(
                                ExponentialBackoff::doubling_range_secs((0.1, 90.0)),
                            );

                            timer_fill_playlist.defer_with(backoff.swap_next());
                        }
                    }
                    LoopEvent::BucketFillStart => {
                        let spigot = pusher.get_spigot_mut();
                        let needs = get_bucket_fill_needs(spigot).collect();

                        bucket_fill_tx.queue(needs);
                    }
                    LoopEvent::BucketFillResponse(response) => match response {
                        bucket_fill::Response::Fill(result) => {
                            let spigot = pusher.get_spigot_mut();
                            apply_bucket_fill_results([result], spigot)
                                .map_err(LoopErrorKind::FillBuckets)?;

                            // only fill buckets when explicitly activated
                            // (NOT running `timer_fill_bucket.set_active(true)`)
                        }
                        bucket_fill::Response::End => {
                            // NOTE: causes update to playlist, BUT cannot delay playlist upkeep
                            timer_fill_playlist.set_immediate();
                        }
                    },
                    LoopEvent::SpigotCmd(cmd) => {
                        cmd.run(&mut pusher);

                        // causes update to buckets
                        timer_fill_bucket.defer();
                        timer_fill_bucket.set_active(true);
                    }
                    LoopEvent::VlcCmd(cmd) => {
                        let result = vlc_act_tx.send_cmd(cmd);
                        warn_queue_failed(result, "VLC command");
                    }
                    LoopEvent::VlcCmdResponse { modifies_playlist } => {
                        if modifies_playlist {
                            // causes update to playlist
                            timer_fill_playlist.set_immediate();
                        }
                    }
                }
            }
            Ok(())
        })();

        eprintln!("WAIT for bucket_fill_thread to join");
        warn_thread_panic(
            bucket_fill_thread.join_and_drop(bucket_fill_tx),
            "bucket fill",
        );

        eprintln!("WAIT for vlc_act_thread to join");
        warn_thread_panic(vlc_act_thread.join_and_drop(vlc_act_tx), "VLC act");

        loop_result.map_err(|kind| LoopError { kind })
    }
}

fn warn_queue_failed<E>(result: Result<(), E>, kind: &str)
where
    E: Into<eyre::Report>,
{
    if let Err(e) = result {
        let error = eyre::eyre!(e);
        tracing::warn!(?error, "failed to queue {kind}");
    }
}
fn warn_thread_panic(result: std::thread::Result<()>, kind: &str) {
    if let Err(error) = result {
        tracing::warn!(?error, "panic in {kind} thread");
    }
}

struct ExponentialBackoff {
    range_seconds: (f64, f64),
    multiplier: f64,
}
impl ExponentialBackoff {
    fn doubling_range_secs(range_seconds: (f64, f64)) -> Self {
        Self {
            range_seconds,
            multiplier: 2.0,
        }
    }
    fn swap_next(&mut self) -> std::time::Duration {
        let Self {
            range_seconds: (min_secs, max_secs),
            multiplier,
        } = *self;

        let next_secs = (min_secs * multiplier).max(min_secs).min(max_secs);

        let Self {
            range_seconds: (current, _),
            ..
        } = self;
        std::time::Duration::from_secs_f64(std::mem::replace(current, next_secs))
    }
}

#[derive(Debug)]
struct HintRetryableError;
fn is_retryable<E>(err: &PusherError<E, vlc_http_ureq::Error>) -> Option<HintRetryableError>
where
    E: std::error::Error + Send + Sync + 'static,
{
    use crate::pusher::ErrorKind;
    use vlc_http_ureq::ureq_crate::Error as UreqError;

    let vlc_sync_err = match err.kind() {
        ErrorKind::HttpRunner(error) => error,
        ErrorKind::BeetPath(_) | ErrorKind::Observer(_) => return None,
    };

    let ureq_err = vlc_sync_err.try_as_endpoint_err()?.try_as_ureq()?;

    match ureq_err {
        UreqError::Io(_)
        | UreqError::Timeout(_)
        | UreqError::HostNotFound
        | UreqError::ConnectionFailed => Some(HintRetryableError),
        UreqError::StatusCode(_)
        | UreqError::Http(_)
        | UreqError::BadUri(_)
        | UreqError::Protocol(_)
        | UreqError::RedirectFailed
        | UreqError::InvalidProxyUrl
        | UreqError::BodyExceedsLimit(_)
        | UreqError::TooManyRedirects
        | UreqError::Tls(_)
        | UreqError::RequireHttpsOnly(_)
        | UreqError::LargeResponseHeader(_, _)
        | UreqError::ConnectProxyFailed(_)
        | UreqError::TlsRequired
        | UreqError::Other(_)
        | _ => None,
    }
}

impl LoopEventSpigotCmd {
    fn run<R>(self, pusher: &mut BeetPusher<'_, R>) {
        use crate::pipe_exec::{ErrorKind, ResponseData};

        let Self {
            reply_to: OpaqueDebug(reply_to),
            cmd,
        } = self;

        let spigot = pusher.get_spigot_mut();
        let result = match spigot.modify_and_get_created_path(cmd.into()) {
            Ok(Some(path)) => Ok(ResponseData::NodeAdded { path }),
            Ok(None) => Ok(ResponseData::PassNoData),
            Err(e) => Err(ErrorKind::SpigotError(e)),
        };
        let _ = reply_to.send(result);
    }
}
impl LoopEventVlcCmd {
    fn run(self, vlc_driver: &mut VlcDriver<HttpRunner>) {
        use crate::pipe_exec::{ErrorKind, ResponseData};
        use vlc_http::sync::EndpointRequestor as _;

        let Self {
            reply_to: OpaqueDebug(reply_to),
            cmd,
        } = self;

        let cmd = vlc_http::Command::from(cmd);

        let response = vlc_driver.http_runner.request(cmd.into());

        let result = match response {
            Ok(response) => {
                vlc_driver.client_state.update(response);
                Ok(ResponseData::PassNoData)
            }
            Err(e) => Err(ErrorKind::VlcRequest(e)),
        };
        let _ = reply_to.send(result);
    }
}

/// Error from the main [`CommandLoop::run`]
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct LoopError<E, R> {
    kind: LoopErrorKind<E, R>,
}
#[derive(Debug, thiserror::Error)]
enum LoopErrorKind<E, R> {
    #[error("failed to pull items from the spigot")]
    PullItems(#[source] crate::FillDeterminedError<R>),
    #[error("failed to fill spigot bucket items")]
    FillBuckets(#[source] crate::beet::FillError<std::io::Error>),
    #[error("failed to push playlist update")]
    PlaylistUpdate(#[source] crate::pusher::Error<E, vlc_http_ureq::Error>),
}

/// Accepts events to add to the command loop
pub struct EventSender {
    loop_tx: crossbeam_channel::Sender<LoopEvent>,
}
impl EventSender {
    /// Sends the event with a timeout
    ///
    /// # Errors
    ///
    /// Returns an error if the send times out or is disconnected
    fn send_timeout(&self, event: LoopEvent) -> Result<(), crate::pipe_exec::ErrorKind> {
        use crate::pipe_exec::ErrorKind;
        use crossbeam_channel::SendTimeoutError;

        const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

        let Self { loop_tx } = self;
        loop_tx
            .send_timeout(event, SEND_TIMEOUT)
            .map_err(|e| match e {
                SendTimeoutError::Disconnected(_) => ErrorKind::LoopShutdown,
                SendTimeoutError::Timeout(_) => ErrorKind::InternalTimeout,
            })
    }
    /// Documents the site of sending a shutdown request to the [`CommandLoop`], consuming this sender handle
    pub fn send_shutdown(self) {
        drop(self);
    }
}
impl Drop for EventSender {
    fn drop(&mut self) {
        // send (with a timeout to avoid hang in Drop)
        let _ = self.send_timeout(Shutdown.into());
    }
}

/// Event runnable by the [`CommandLoop`] (sent via [`EventSender`])
#[derive(Debug)]
enum LoopEvent {
    Shutdown(Shutdown),
    MaintainPlaylistStart,
    MaintainPlaylistResponse(vlc_act::PlaylistResponse<vlc_http_ureq::Error>),
    BucketFillStart,
    BucketFillResponse(bucket_fill::Response),
    SpigotCmd(LoopEventSpigotCmd),
    VlcCmd(LoopEventVlcCmd),
    VlcCmdResponse { modifies_playlist: bool },
}
#[derive(Debug)]
struct LoopEventSpigotCmd {
    reply_to: OpaqueDebug<oneshot::Sender<crate::pipe_exec::ResponseResultInner>>,
    cmd: SpigotCmd,
}
#[derive(Debug)]
struct LoopEventVlcCmd {
    reply_to: OpaqueDebug<oneshot::Sender<crate::pipe_exec::ResponseResultInner>>,
    cmd: VlcCmd,
}
impl From<Shutdown> for LoopEvent {
    fn from(value: Shutdown) -> Self {
        Self::Shutdown(value)
    }
}

/// Helper to bridge derive for fields that are not [`Debug`]
struct OpaqueDebug<T>(pub T);
impl<T> From<T> for OpaqueDebug<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
impl<T> std::fmt::Debug for OpaqueDebug<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "_")
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
    fn take_ready(&mut self) -> bool {
        let is_ready = self.is_ready();
        if is_ready {
            self.set_active(false);
        }
        is_ready
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
    let reply_to = reply_to.into();

    let event = match cmd {
        PipeCommand::Spigot(cmd) => LoopEvent::SpigotCmd(LoopEventSpigotCmd { reply_to, cmd }),
        PipeCommand::Vlc(cmd) => LoopEvent::VlcCmd(LoopEventVlcCmd { reply_to, cmd }),
    };

    loop_tx.send_timeout(event).map_err(make_err)?;

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

#[cfg(test)]
mod tests_backoff {
    use super::ExponentialBackoff;

    #[test]
    fn backoff() {
        let mut backoff = ExponentialBackoff::doubling_range_secs((1.0, 300.0));
        let mut intervals = vec![];
        loop {
            let last = intervals.last().copied();

            let new = backoff.swap_next().as_secs_f64();
            intervals.push(new);

            let Some(last) = last else {
                continue;
            };

            #[expect(
                clippy::float_cmp,
                reason = "find saturating point (matching input parameter)"
            )]
            if new == last {
                break;
            }
        }

        assert_eq!(
            intervals,
            &[
                1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 300.0, 300.0
            ]
        );
    }
}
