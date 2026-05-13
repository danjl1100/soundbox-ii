// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Pushes tracks from `beet` to VLC, with a minimal (read "nonexistent") user interface
//!
//! Proof of concept for pushing a simple beet query to VLC, with id tracking

use crate::config_file::ConfigFile;
use arg_util::ConfigFileOpen as _;
use beet_pusher::{BeetItem, BeetPusher, Shutdown, fill_buckets, pipe_exec::SpigotCmd};
use clap::Parser;
use eyre::Context as _;
use std::{borrow::Cow, path::PathBuf, sync::mpsc::RecvTimeoutError};

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(flatten)]
    auth_args_and_file: vlc_http_auth_clap::ClapAuthInputAndFile,
    #[clap(long)]
    config_file: Option<std::path::PathBuf>,
    /// Script file to use for the bucket spigot sequencer
    #[clap(long)]
    spigot_script: Option<std::path::PathBuf>,
    /// If set, only print the bucket spigot setup then exit
    #[clap(long)]
    debug_items: bool,
    #[clap(long)]
    json: bool,
}

#[expect(clippy::too_many_lines, reason = "TODO cleanup modules in main")]
fn main() -> eyre::Result<()> {
    // NOTE: **DO NOT** quote arguments, as there is no interpreter to strip the quotes
    const DEFAULT_SCRIPT: &str = "";
    // "
    // add-joint .

    // add-bucket .0
    // set-order-type .0.0 shuffle
    // set-filters .0.0 added:2020.. grouping::^$

    // add-bucket .0
    // set-order-type .0.1 shuffle
    // set-filters .0.1 grouping::1|2|3|4|5 has_lyrics::^$
    // ";

    init_tracing();

    let Args {
        auth_args_and_file,
        config_file,
        spigot_script,
        debug_items,
        json,
    } = Args::parse();

    let mut http_runner = {
        let auth = auth_args_and_file.merge()?;
        let auth = vlc_http::Auth::new(auth)?;
        vlc_http_ureq::HttpRunner::new(auth)
    };

    // TODO handle weirder requests like:
    // <file:///clone/wilbur_dan/beet/Music/Louie%20Zong/3%/01%20That%20Someone%20Is%20You.mp3>
    let script = spigot_script.map_or_else(
        || {
            tracing::info!("Using default script");
            Ok(Cow::Borrowed(DEFAULT_SCRIPT))
        },
        |spigot_script| {
            tracing::info!("Reading script file");
            std::fs::read_to_string(&spigot_script)
                .with_context(|| format!("failed to read script file {}", spigot_script.display()))
                .map(Cow::Owned)
        },
    )?;

    // TODO delete unused diagnostic
    if debug_items {
        let mut beet_cmd = beet_pusher::BeetCommand::new_beet();
        let mut spigot = setup_spigot(&mut beet_cmd, &script)?;
        let view = spigot.view_table_default();
        eprintln!("{view}");
        let rng = &mut bucket_spigot::order::ErrorRng(&mut rand::thread_rng());
        for _ in 0..50 {
            let peeked = spigot.peek(rng, 1)?;
            eprintln!("{:?}", peeked.items());
            spigot.finalize_peeked(peeked.accept_into_inner());
        }
        return Ok(());
    }

    let config_file = config_file.unwrap_or_else(|| PathBuf::from("beet-pusher.config.toml"));
    let config_file = match ConfigFile::open(&config_file) {
        Ok(config_file) => config_file,
        Err(error) if error.is_missing_file() => {
            let template_file = ConfigFile::write_template_for_file(config_file)?;
            eyre::bail!(
                "config file not found, wrote template to {}",
                template_file.display()
            )
        }
        Err(error) => Err(error)?,
    };
    let ConfigFile {
        base_url,
        publish_id_file,
        beet,
    } = config_file;

    let mut now_playing_observer = move |item: &BeetItem| {
        let beet_id = item.get_beet_id();
        let path = item.get_path().as_str();

        eprintln!("Now playing id={beet_id}: {path}");

        publish_id_file
            .as_ref()
            .map(|dest| now_playing_observer::write_now_playing_file(dest, item))
            .transpose()?;

        Ok::<_, now_playing_observer::PublishError>(())
    };

    let rng = &mut bucket_spigot::order::ErrorRng(&mut rand::thread_rng());

    let mut beet_cmd = beet.map_or_else(beet_pusher::BeetCommand::new_beet, |p| {
        beet_pusher::BeetCommand::new(std::borrow::Cow::Owned(p.into_os_string()))
    });
    let spigot = setup_spigot(&mut beet_cmd, &script)?;

    let mut pusher = BeetPusher::new(rng, spigot, base_url);
    // let mut client_state = vlc_http::ClientState::new();

    let (loop_tx, loop_rx) = std::sync::mpsc::sync_channel(1);
    if json {
        std::thread::spawn(move || {
            match pipe_cmd_loop(&loop_tx) {
                Ok(()) => eprintln!("end of input on stdin"),
                Err(e) => eprintln!("pipe_cmd_loop fatal error: {e:?}"),
            }
            let _ = loop_tx.send(Shutdown.into());
        });
    }

    // TODO add a "determined holder" concept, to make it easy to:
    // 1. Peek a bunch, update spigot
    // 2. Load into VLC, retrieve "after current" items
    // 3. Pop from the "determined" holder
    // 4. Repeat from step 1, only peeking what is needed
    // ---> Prototype as a struct here, the move to bucket_spigot::order if it's generally useful

    {
        const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(100);
        const FILL_PLAYLIST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

        let mut timer_fill_playlist = DebounceTask::new(FILL_PLAYLIST_INTERVAL);
        let mut timer_fill_bucket = DebounceTask::new(SLEEP_DURATION);
        // only fill buckets when explicitly activated
        timer_fill_bucket.set_active(false);
        loop {
            let event = if timer_fill_playlist.is_ready() {
                // highest priority - feed the externally-advancing VLC
                Ok(LoopEvent::MaintainPlaylist)
            } else if timer_fill_bucket.is_ready() {
                // next priority - update to reflect the user command
                Ok(LoopEvent::MaintainBuckets)
            } else {
                // lowest - process new commands
                loop_rx.recv_timeout(SLEEP_DURATION)
            };
            match event {
                Ok(loop_event) => match loop_event {
                    LoopEvent::Shutdown(Shutdown) => break,
                    LoopEvent::MaintainPlaylist => {
                        if !pusher.get_spigot_mut().is_empty() {
                            tracing::trace!("FILL DETERMINED");
                            pusher.fill_determined()?;
                        }
                        tracing::trace!("PLAYLIST UPDATE");
                        pusher.push_playlist_update(
                            &mut http_runner,
                            Some(&mut now_playing_observer),
                        )?;

                        // requires constant maintenance (VLC client self-advances)
                        timer_fill_playlist.defer();
                    }
                    LoopEvent::MaintainBuckets => {
                        tracing::trace!("FILL BUCKETS");

                        let spigot = pusher.get_spigot_mut();
                        fill_buckets(&mut beet_cmd, spigot)?;

                        timer_fill_bucket.set_active(false);

                        // NOTE: causes update to playlist, BUT cannot delay playlist upkeep
                        timer_fill_playlist.set_immediate();
                    }
                    LoopEvent::SpigotCmd { reply_to, cmd } => {
                        use beet_pusher::pipe_exec::{Error, ResponseData};
                        let spigot = pusher.get_spigot_mut();
                        tracing::trace!(?cmd);
                        let result = match spigot.modify_and_get_created_path(cmd.into()) {
                            Ok(Some(path)) => Ok(ResponseData::NodeAdded { path }),
                            Ok(None) => Ok(ResponseData::PassNoData),
                            Err(e) => Err(Error::SpigotError(e)),
                        };
                        let _ = reply_to.send(result);

                        // causes update to buckets
                        timer_fill_bucket.defer();
                        timer_fill_bucket.set_active(true);
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
    }

    tracing::trace!("END OF BEET-PUSHER MAIN");

    Ok(())
}

enum LoopEvent {
    Shutdown(Shutdown),
    MaintainPlaylist,
    MaintainBuckets,
    SpigotCmd {
        reply_to: oneshot::Sender<beet_pusher::pipe_exec::ResponseResultInner>,
        cmd: SpigotCmd,
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
    /// Blocks `is_ready` when `active` is false
    fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
    /// Sets the timer to ready when next active
    fn set_immediate(&mut self) {
        self.last_run = None;
    }
}

fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}
fn setup_spigot(
    beet_cmd: &mut beet_pusher::BeetCommand,
    script: &str,
) -> eyre::Result<bucket_spigot::Network<BeetItem, String>> {
    use bucket_spigot::Network;

    let mut spigot = Network::from_commands_str_whitespace(script)?;
    fill_buckets(beet_cmd, &mut spigot)?;

    // NOTE: Empty is a valid startup state
    // if spigot.is_empty() {
    //     eyre::bail!("no items for the selected filters, see RUST_LOG=trace output above");
    // }

    Ok(spigot)
}

fn pipe_cmd_loop(loop_tx: &std::sync::mpsc::SyncSender<LoopEvent>) -> eyre::Result<()> {
    use beet_pusher::pipe_exec::ResponseOut;

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
fn pipe_cmd(
    loop_tx: &std::sync::mpsc::SyncSender<LoopEvent>,
    line: String,
) -> beet_pusher::pipe_exec::ResponseResult {
    const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

    let beet_pusher::pipe_exec::CommandIn { seq, cmd } = serde_json::from_str(&line)
        .map_err(|e| beet_pusher::pipe_exec::Error::new_invalid_command(line, e))?;
    match cmd {
        beet_pusher::pipe_exec::Command::Spigot(cmd) => {
            let (reply_to, rx) = oneshot::channel();
            let _ = loop_tx.send(LoopEvent::SpigotCmd { reply_to, cmd });
            match rx.recv_timeout(RESPONSE_TIMEOUT) {
                Ok(result) => result.map(|data| (seq, data)),
                Err(e) => match e {
                    oneshot::RecvTimeoutError::Timeout
                    | oneshot::RecvTimeoutError::Disconnected => {
                        Err(beet_pusher::pipe_exec::Error::InternalTimeout)
                    }
                },
            }
        }
    }
}

mod now_playing_observer {
    // TODO: rewrite, should be only 1 file open to check contents then overwrite if just a number

    use beet_pusher::BeetItem;

    pub fn write_now_playing_file(
        publish_id_file: impl AsRef<std::path::Path>,
        item: &BeetItem,
    ) -> Result<(), PublishError> {
        let publish_id_file = publish_id_file.as_ref();

        let make_error = |kind| PublishError {
            publish_id_file: publish_id_file.to_path_buf(),
            kind,
        };

        // check that the file contains a (short) number (e.g. not irreplaceable data)
        match read_as_single_u16(publish_id_file) {
            Ok(None) => Err(make_error(ErrorKind::ExistingFileNonNumeric)),
            Ok(Some(_)) => {
                //  OK to overwrite a file containing a single u16
                Ok(())
            }
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {
                    // OK to write, file does not appear to exist
                    // FIXME: TOCTOU issue
                    Ok(())
                }
                _ => Err(make_error(ErrorKind::ExistingFileRead(err))),
            },
        }?;

        let beet_id = item.get_beet_id();

        let contents = format!("{beet_id}");
        let contents = contents.as_bytes();
        std::fs::write(publish_id_file, contents)
            .map_err(ErrorKind::Write)
            .map_err(make_error)?;

        Ok(())
    }

    fn read_as_single_u16(path: &std::path::Path) -> Result<Option<u16>, std::io::Error> {
        let old_contents = std::fs::read_to_string(path)?;
        let mut lines = old_contents.lines();

        let Some(first_line) = lines.next() else {
            return Ok(None);
        };
        let number = first_line.parse().ok();

        let second_line = lines.next();
        if matches!(second_line, Some(second_line) if !second_line.trim().is_empty()) {
            // second line not empty --> report "not a simple number"
            Ok(None)
        } else {
            Ok(number)
        }
    }

    #[derive(Debug)]
    pub struct PublishError {
        publish_id_file: std::path::PathBuf,
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Write(std::io::Error),
        ExistingFileNonNumeric,
        ExistingFileRead(std::io::Error),
    }
    impl std::error::Error for PublishError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            use ErrorKind as Kind;
            match &self.kind {
                Kind::Write(error) | Kind::ExistingFileRead(error) => Some(error),
                Kind::ExistingFileNonNumeric => None,
            }
        }
    }
    impl std::fmt::Display for PublishError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use ErrorKind as Kind;

            let Self {
                publish_id_file,
                kind,
            } = self;
            let description = match kind {
                Kind::Write(_error) => "failed to write",
                Kind::ExistingFileNonNumeric => "refusing to overwrite non-numeric",
                Kind::ExistingFileRead(_error) => "failed to sanity-check read",
            };
            write!(
                f,
                "{description} publish_id_file: {}",
                publish_id_file.display()
            )
        }
    }
}

mod config_file {
    use arg_util::ConfigFileWrite as _;
    use beet_pusher::BaseUrl;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub(super) struct ConfigFile {
        pub base_url: BaseUrl,
        // If specified, writes the "now playing" ID to a text file for other scripts to pickup
        pub publish_id_file: Option<std::path::PathBuf>,
        pub beet: Option<std::path::PathBuf>,
    }

    impl ConfigFile {
        pub fn write_template_for_file(
            path: impl AsRef<std::path::Path>,
        ) -> Result<std::path::PathBuf, arg_util::config_file::ErrorWrite> {
            Self {
                base_url: BaseUrl(
                    "file:///path/to/beets/media/folder/"
                        .parse()
                        .expect("default base_url should parse"),
                ),
                publish_id_file: Some("current_item_id.txt".into()),
                beet: Some("/path/to/usr/bin/beet".into()),
            }
            .write_template_for_file(path)
        }
    }
}
