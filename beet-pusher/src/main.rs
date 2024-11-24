// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Pushes tracks from `beet` to VLC, with a minimal (read "nonexistent") user interface
//!
//! Proof of concept for pushing a simple beet query to VLC, with id tracking

use clap::Parser;
use determined::Determined;
use todo_move_to_a_beet_lib::{query_beet, BeetItem};
use vlc_http::action::TargetPlaylistItems;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(flatten)]
    auth: vlc_http::clap::AuthInput,
    // TODO publish_file, to write the "now playing" ID to a text file for other scripts to pickup
}

fn main() -> eyre::Result<()> {
    const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(1000);

    tracing_subscriber::fmt::init();

    let Args { auth } = Args::parse();
    let auth = vlc_http::Auth::new(auth.into())?;

    let spigot = setup_spigot()?;

    let rng = &mut rand::thread_rng();
    let state = vlc_http::ClientState::new();
    let http_runner = vlc_http::http_runner::ureq::HttpRunner::new(auth);

    let mut pusher = BeetPusher {
        spigot,
        rng,
        client: Client {
            state,
            last_state_sequence: None,
        },
        http_runner,
        determined: Determined::default(),
    };

    // TODO add a "determined holder" concept, to make it easy to:
    // 1. Peek a bunch, update spigot
    // 2. Load into VLC, retrieve "after current" items
    // 3. Pop from the "determined" holder
    // 4. Repeat from step 1, only peeking what is needed
    // ---> Prototype as a struct here, the move to bucket_spigot::order if it's generally useful
    loop {
        pusher.fill_determined()?;
        pusher.push_playlist_update()?;

        std::thread::sleep(SLEEP_DURATION);
    }
}

type UreqError = vlc_http::http_runner::ureq::Error;
type ExhaustResult<'a, T> =
    Result<<T as vlc_http::Pollable>::Output<'a>, vlc_http::sync::Error<T, UreqError>>;

struct BeetPusher<'a, R> {
    spigot: bucket_spigot::Network<BeetItem, String>,
    rng: &'a mut R,
    client: Client,
    http_runner: vlc_http::http_runner::ureq::HttpRunner,
    determined: Determined<BeetItem>,
}
struct Client {
    state: vlc_http::ClientState,
    last_state_sequence: Option<vlc_http::client_state::ClientStateSequence>,
}
impl<R: rand::RngCore> BeetPusher<'_, R> {
    fn exhaust_pollable<T>(&mut self, query: T) -> ExhaustResult<'_, T>
    where
        T: vlc_http::Pollable,
    {
        const MAX_ENDPOINTS_PER_ACTION: usize = 100;
        let (output, seq) = vlc_http::sync::exhaust_pollable(
            query,
            &mut self.client.state,
            &mut self.http_runner,
            MAX_ENDPOINTS_PER_ACTION,
        )?;
        if let Some(seq) = seq {
            self.client.last_state_sequence = Some(seq);
        }
        Ok(output)
    }
    fn fill_determined(&mut self) -> eyre::Result<()> {
        let peek_len = match self.determined.items().len() {
            len @ 0..=0 => Some(1 - len),
            1 => None,
            2.. => unreachable!("determined should be 1 item or fewer"),
        };

        if let Some(peek_len) = peek_len {
            let peeked = self.spigot.peek(self.rng, peek_len)?;
            if peeked.items().len() != peek_len {
                let view = self.spigot.view_table_default();
                unreachable!(
                    "insufficient items in spigot count = {found}, expected {expected}:\n{view}",
                    found = peeked.items().len(),
                    expected = peek_len,
                );
            }
            let () = self.determined.modify_gen_urls(gen_path_url, |dest| {
                dest.extend(peeked.items().iter().map(|&item| item.clone()));
            })?;
            self.spigot.finalize_peeked(peeked.accept_into_inner());

            tracing::debug!(
                items = ?self.determined.items(),
                "Selected new desired items",
            );
        }
        assert_eq!(
            self.determined.len(),
            1,
            "determine should be 1 item after peek"
        );
        Ok(())
    }
    fn push_playlist_update(&mut self) -> eyre::Result<()> {
        let target = TargetPlaylistItems::new()
            .set_urls(self.determined.urls().to_vec()) // FIXME cloning to vec feels so wrong...
            .set_keep_history(5);

        let action = vlc_http::action::Action::set_playlist_query_matched(
            target,
            self.client.state.get_ref(),
        );
        let output = self.exhaust_pollable(action)?;
        let output_len = output.len();
        if output_len < self.determined.len() {
            let () = self.determined.modify_gen_urls(gen_path_url, |dest| {
                // FIXME this would be terrible (~N^2?) if expected len >> 2
                while dest.len() > output_len {
                    let removed = dest.remove(0);
                    println!(
                        "Now playing id={beet_id}: {path}",
                        beet_id = removed.get_beet_id(),
                        path = removed.get_path(),
                    );
                }
            })?;
        }
        Ok(())
    }
}

impl<R> std::fmt::Debug for BeetPusher<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct DebugAsDisplay<T>(T);
        impl<T> std::fmt::Debug for DebugAsDisplay<T>
        where
            T: std::fmt::Display,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                <T as std::fmt::Display>::fmt(&self.0, f)
            }
        }

        let Self {
            spigot,
            rng: _,
            client:
                Client {
                    state,
                    last_state_sequence,
                },
            http_runner: _,
            determined,
        } = self;
        f.debug_struct("BeetPusher")
            .field("spigot", &DebugAsDisplay(spigot.view_table_default()))
            .field("client.state", state)
            .field("client.last_state_sequence", last_state_sequence)
            .field("determined.items", &determined.items())
            .field("determined.urls", &determined.urls())
            .finish()
    }
}

mod determined {
    pub struct Determined<T> {
        items: Vec<T>,
        urls: Vec<url::Url>,
    }
    impl<T> Default for Determined<T> {
        fn default() -> Self {
            Self {
                items: vec![],
                urls: vec![],
            }
        }
    }
    impl<T> Determined<T> {
        pub fn items(&self) -> &[T] {
            &self.items
        }
        pub fn urls(&self) -> &[url::Url] {
            &self.urls
        }
        pub fn len(&self) -> usize {
            self.items.len()
        }
        pub fn modify_gen_urls<U, E>(
            &mut self,
            gen_url_fn: impl FnMut(&T) -> Result<url::Url, E>,
            modify_fn: impl FnOnce(&mut Vec<T>) -> U,
        ) -> Result<U, E> {
            let mut result = Ok(modify_fn(&mut self.items));
            match self.items.iter().map(gen_url_fn).collect() {
                Ok(new_urls) => self.urls = new_urls,
                Err(err) => {
                    // failed to create URLs, clear items for consistent state
                    self.items.clear();
                    self.urls.clear();
                    result = Err(err);
                }
            }
            assert_eq!(
                self.items.len(),
                self.urls.len(),
                "determined items/urls should match lengths"
            );
            result
        }
    }
}

fn gen_path_url(item: &BeetItem) -> eyre::Result<url::Url> {
    // TODO tunable base path, add tests for Windows path conversion to URL
    const BASE_URL_STR: &str = "file:///clone/wilbur_dan/";

    let base = url::Url::parse(BASE_URL_STR)
        .map_err(|e| eyre::Report::new(e).wrap_err(format!("static base URL {BASE_URL_STR:}")))?;

    let path = item.get_path();
    let path = path.strip_prefix('/').unwrap_or(path);

    base.join(path).map_err(|e| {
        eyre::Report::new(e).wrap_err(format!("joining base {base} to {path:?} should succeed"))
    })
}

fn setup_spigot() -> eyre::Result<bucket_spigot::Network<BeetItem, String>> {
    use bucket_spigot::{
        order::OrderType,
        path::{Path, PathRef},
        ModifyCmd, Network,
    };
    const ROOT: Path = bucket_spigot::path::Path::empty();
    let bucket_path: Path = ".0".parse().expect("valid path .0");

    let mut spigot = Network::default();
    let init_commands = {
        // TODO: use the Network creation script... Luke!
        vec![
            ModifyCmd::AddBucket { parent: ROOT },
            // NOTE: this step not strictly necessary, but want to test the beet params roundtrip
            // through bucket_spigot nodes
            ModifyCmd::SetFilters {
                path: bucket_path.clone(),
                new_filters: vec!["year:2024".to_owned()],
            },
            ModifyCmd::SetOrderType {
                path: bucket_path,
                new_order_type: OrderType::Shuffle,
            },
        ]
    };
    for cmd in init_commands {
        spigot.modify(cmd)?;
    }

    let buckets: Vec<_> = spigot
        .get_buckets_needing_fill()
        .map(PathRef::to_owned)
        .collect();

    for bucket in buckets {
        let filters = spigot
            .get_filters(bucket.as_ref())
            .expect("path should be valid for bucket needing fill")
            .into_iter()
            .flat_map(|filter_set| filter_set.iter().cloned());
        let new_contents = query_beet(filters)?;
        spigot.modify(ModifyCmd::FillBucket {
            bucket,
            new_contents,
        })?;
    }

    Ok(spigot)
}

mod todo_move_to_a_beet_lib {
    pub use self::beet_item::BeetItem;
    use std::{borrow::Cow, io::BufRead, process::Command};

    pub(super) fn query_beet(
        filters: impl Iterator<Item = String>,
    ) -> Result<Vec<BeetItem>, Error> {
        let make_error = |kind| Error { kind };

        let output = Command::new("beet")
            .arg("ls")
            .arg("-f")
            .arg("$id=$path")
            .args(filters)
            .output()
            .map_err(ErrorKind::Spawn)
            .map_err(make_error)?;

        if !output.stderr.is_empty() {
            return Err(make_error(ErrorKind::Stderr {
                stderr_str: String::from_utf8_lossy(&output.stderr).to_string(),
            }));
        }

        if !output.status.success() {
            return Err(make_error(ErrorKind::ExitFail {
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            }));
        }

        output
            .stdout
            .lines()
            .map(|line| {
                let line = line.map_err(ErrorKind::Read)?;
                line.parse()
                    .map_err(|error| ErrorKind::InvalidLine { line, error })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(make_error)
    }

    #[derive(Debug)]
    pub(super) struct Error {
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Spawn(std::io::Error),
        Read(std::io::Error),
        Stderr {
            stderr_str: String,
        },
        ExitFail {
            code: Option<i32>,
            stdout: String,
        },
        InvalidLine {
            line: String,
            error: beet_item::Error,
        },
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            use ErrorKind as E;
            match &self.kind {
                E::Spawn(error) | E::Read(error) => Some(error),
                E::Stderr { stderr_str: _ } | E::ExitFail { .. } => None,
                E::InvalidLine { error, .. } => Some(error),
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            #[rustfmt::skip]
            fn funnel<'a, F: Fn(usize, &'a str) -> Cow<'a, str>>(f: F) -> F { f }
            let max_len = funnel(|max, raw_input: &str| {
                if raw_input.len() > max {
                    Cow::Owned(format!("{} ...", &raw_input[..max]))
                } else {
                    Cow::Borrowed(raw_input)
                }
            });
            let (description, details) = match &self.kind {
                ErrorKind::Spawn(_) => ("failed to spawn", None),
                ErrorKind::Read(_) => ("failed to read from", None),
                ErrorKind::Stderr { stderr_str } => {
                    ("stderr output from", Some(Cow::Borrowed(&**stderr_str)))
                }
                ErrorKind::ExitFail { code, stdout } => {
                    let stdout = max_len(200, stdout);
                    let details = match code {
                        Some(code) => Cow::Owned(format!("[code {code}] {stdout}")),
                        None => stdout,
                    };
                    ("failure status code from", Some(details))
                }
                ErrorKind::InvalidLine { line, error: _ } => {
                    let line = max_len(80, line);
                    ("invalid output line from", Some(line))
                }
            };
            write!(f, "{description} beet command")?;
            if let Some(details) = details {
                write!(f, ": {details}")?;
            }
            Ok(())
        }
    }

    mod beet_item {
        use std::str::FromStr;

        const SEPARATOR: &str = "=";

        #[derive(Clone, Debug)]
        pub struct BeetItem {
            beet_id: u64,
            // NOTE: not `PathBuf` because we already entered UTF-8 land by parsing Beet output
            //       The string may need further modifications to represent a real path
            path: String,
        }
        impl BeetItem {
            pub fn get_beet_id(&self) -> u64 {
                self.beet_id
            }
            pub fn get_path(&self) -> &str {
                &self.path
            }
        }
        impl FromStr for BeetItem {
            type Err = Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let Some((beet_id, path)) = s.split_once(SEPARATOR) else {
                    return Err(Error {
                        kind: ErrorKind::MissingSeparator,
                    });
                };
                let beet_id = beet_id
                    .parse()
                    .map_err(ErrorKind::InvalidId)
                    .map_err(|kind| Error { kind })?;
                let path = path.into();
                Ok(Self { beet_id, path })
            }
        }
        impl std::fmt::Display for BeetItem {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self { beet_id, path } = self;
                write!(f, "{beet_id}{SEPARATOR}{path}")
            }
        }

        #[derive(Debug)]
        pub struct Error {
            kind: ErrorKind,
        }
        #[derive(Debug)]
        enum ErrorKind {
            MissingSeparator,
            InvalidId(std::num::ParseIntError),
        }
        impl std::error::Error for Error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match &self.kind {
                    ErrorKind::MissingSeparator => None,
                    ErrorKind::InvalidId(error) => Some(error),
                }
            }
        }
        impl std::fmt::Display for Error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let description = match self.kind {
                    ErrorKind::MissingSeparator => "missing separator",
                    ErrorKind::InvalidId(_) => "invalid id number",
                };
                write!(f, "{description}")
            }
        }
    }
}
