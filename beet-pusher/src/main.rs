// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Pushes tracks from `beet` to VLC, with a minimal (read "nonexistent") user interface
//!
//! Proof of concept for pushing a simple beet query to VLC, with id tracking

use crate::config_file::ConfigFile;
use clap::Parser;
use determined::Determined;
use path_url::BaseUrl;
use std::path::PathBuf;
use todo_move_to_a_beet_lib::{query_beet, BeetItem};
use tracing::{debug, info};
use vlc_http::goal::TargetPlaylistItems;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(flatten)]
    auth: vlc_http::clap::AuthInput,
    #[clap(long)]
    config_file: Option<std::path::PathBuf>,
}

fn main() -> eyre::Result<()> {
    const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(1000);

    tracing_subscriber::fmt::init();

    // TODO delete unused diagnostic
    if false {
        let mut spigot = setup_spigot()?;
        let view = spigot.view_table_default();
        println!("{view}");
        let rng = &mut rand::thread_rng();
        for _ in 0..50 {
            let peeked = spigot.peek(rng, 1)?;
            println!("{:?}", peeked.items());
            spigot.finalize_peeked(peeked.accept_into_inner());
        }
        return Ok(());
    }

    let Args { auth, config_file } = Args::parse();
    let auth = vlc_http::Auth::new(auth.into())?;

    let config_file = config_file.unwrap_or(PathBuf::from("beet-pusher.config.toml"));
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
    } = config_file;

    let now_playing_observer = |item: BeetItem| {
        let beet_id = item.get_beet_id();
        let path = item.get_path();

        println!("Now playing id={beet_id}: {path}");

        publish_id_file
            .as_ref()
            .map(|dest| now_playing_observer::write_now_playing_file(dest, &item))
            .transpose()?;

        Ok::<_, now_playing_observer::PublishError>(())
    };

    let spigot = setup_spigot()?;

    let rng = &mut rand::thread_rng();
    let state = vlc_http::ClientState::new();
    let http_runner = vlc_http::http_runner::ureq::HttpRunner::new(auth);

    let mut pusher = BeetPusher {
        spigot,
        rng,
        client: Client { state },
        http_runner,
        determined: Determined::default(),
        config: Config { base_url },
        now_playing_observer: Some(now_playing_observer),
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

mod now_playing_observer {
    use super::BeetItem;

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

type UreqError = vlc_http::http_runner::ureq::Error;
type ExhaustResult<'a, T> =
    Result<<T as vlc_http::Plan>::Output<'a>, vlc_http::sync::Error<T, UreqError>>;

struct BeetPusher<'a, R, F> {
    spigot: bucket_spigot::Network<BeetItem, String>,
    rng: &'a mut R,
    client: Client,
    http_runner: vlc_http::http_runner::ureq::HttpRunner,
    determined: Determined<BeetItem>,
    config: Config,
    now_playing_observer: Option<F>,
}
struct Client {
    state: vlc_http::ClientState,
}
struct Config {
    base_url: BaseUrl,
}
impl<R: rand::RngCore, F> BeetPusher<'_, R, F> {
    fn complete_plan<T>(&mut self, query: T) -> ExhaustResult<'_, T>
    where
        T: vlc_http::Plan,
    {
        const MAX_ENDPOINTS_PER_ACTION: usize = 100;
        let output = vlc_http::sync::complete_plan(
            query,
            &mut self.client.state,
            &mut self.http_runner,
            MAX_ENDPOINTS_PER_ACTION,
        )?;
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
            let () = self
                .determined
                .modify_gen_urls(&mut self.config.base_url, |dest| {
                    dest.extend(peeked.items().iter().map(|&item| item.clone()));
                })?;
            self.spigot.finalize_peeked(peeked.accept_into_inner());

            debug!(
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
    fn push_playlist_update<E>(&mut self) -> eyre::Result<()>
    where
        F: FnMut(BeetItem) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let target = TargetPlaylistItems::new()
            .set_urls(self.determined.urls().to_vec()) // FIXME cloning to vec feels so wrong...
            .set_keep_history(5);

        let action = self
            .client
            .state
            .build_plan()
            .set_playlist_and_query_matched(target);

        let output = self.complete_plan(action)?;
        let output_len = output.len();
        if output_len < self.determined.len() {
            let () = self
                .determined
                .modify_gen_urls(&mut self.config.base_url, |dest| {
                    // FIXME this would be terrible (~N^2?) if expected len >> 2
                    while dest.len() > output_len {
                        let removed = dest.remove(0);
                        if let Some(now_playing_observer) = &mut self.now_playing_observer {
                            now_playing_observer(removed)?;
                        }
                    }
                    Ok::<_, E>(())
                })??;
        }
        Ok(())
    }
}

impl<R, F> std::fmt::Debug for BeetPusher<'_, R, F> {
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
            client: Client { state },
            http_runner: _,
            determined,
            config: Config { base_url },
            now_playing_observer: _,
        } = self;
        f.debug_struct("BeetPusher")
            .field("spigot", &DebugAsDisplay(spigot.view_table_default()))
            .field("client.state", state)
            .field("determined.items", &determined.items())
            .field("determined.urls", &determined.urls())
            .field("config.base_url", base_url)
            .finish()
    }
}

mod config_file {
    use crate::path_url::BaseUrl;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub(super) struct ConfigFile {
        pub base_url: BaseUrl,
        // If specified, writes the "now playing" ID to a text file for other scripts to pickup
        pub publish_id_file: Option<std::path::PathBuf>,
    }
    impl ConfigFile {
        pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, ErrorOpen> {
            let path = path.as_ref();

            let make_error = |kind| ErrorOpen {
                path: path.to_path_buf(),
                kind,
            };

            let file_contents = std::fs::read_to_string(path)
                .map_err(ErrorOpenKind::Read)
                .map_err(make_error)?;

            toml::from_str(&file_contents)
                .map_err(ErrorOpenKind::Parse)
                .map_err(make_error)
        }
    }

    #[derive(Debug)]
    pub(super) struct ErrorOpen {
        path: std::path::PathBuf,
        kind: ErrorOpenKind,
    }
    #[derive(Debug)]
    enum ErrorOpenKind {
        Read(std::io::Error),
        Parse(toml::de::Error),
    }
    impl ErrorOpen {
        pub fn is_missing_file(&self) -> bool {
            matches!(&self.kind, ErrorOpenKind::Read(err) if matches!(err.kind(), std::io::ErrorKind::NotFound))
        }
    }
    impl std::error::Error for ErrorOpen {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            use ErrorOpenKind as Kind;
            match &self.kind {
                Kind::Read(error) => Some(error),
                Kind::Parse(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for ErrorOpen {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use ErrorOpenKind as Kind;
            let Self { path, kind } = self;
            let description = match kind {
                Kind::Read(_) => "failed to read",
                Kind::Parse(_) => "failed to parse",
            };

            write!(
                f,
                "{description} config file: {path}",
                path = path.display()
            )
        }
    }

    impl ConfigFile {
        pub fn write_template_for_file(
            path: impl AsRef<std::path::Path>,
        ) -> Result<std::path::PathBuf, ErrorWrite> {
            let path = path.as_ref().to_path_buf();
            let template_file = path.with_extension("toml.template");

            let make_error = |kind| ErrorWrite {
                path: template_file.clone(),
                kind,
            };

            let default_config = Self {
                base_url: BaseUrl(
                    "file:///path/to/beets/folder/"
                        .parse()
                        .expect("default base_url should parse"),
                ),
                publish_id_file: Some(std::path::PathBuf::from("current_item_id.txt")),
            };
            let contents =
                toml::to_string(&default_config).expect("default config should serialize");

            std::fs::write(&template_file, contents.as_bytes())
                .map_err(ErrorWriteKind::Write)
                .map_err(make_error)?;

            Ok(template_file)
        }
    }

    #[derive(Debug)]
    pub(super) struct ErrorWrite {
        path: std::path::PathBuf,
        kind: ErrorWriteKind,
    }
    #[derive(Debug)]
    enum ErrorWriteKind {
        Write(std::io::Error),
    }
    impl std::error::Error for ErrorWrite {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorWriteKind::Write(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for ErrorWrite {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { path, kind } = self;
            let description = match kind {
                ErrorWriteKind::Write(_) => "failed to write",
            };
            write!(
                f,
                "{description} config file: {path}",
                path = path.display()
            )
        }
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
            url_source: &mut impl UrlSource<T, Error = E>,
            modify_fn: impl FnOnce(&mut Vec<T>) -> U,
        ) -> Result<U, E> {
            let mut result = Ok(modify_fn(&mut self.items));
            match self
                .items
                .iter()
                .map(|item| url_source.get_url(item))
                .collect()
            {
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

    pub trait UrlSource<T> {
        type Error;
        fn get_url(&mut self, item: &T) -> Result<url::Url, Self::Error>;
    }
    impl<F, T, E> UrlSource<T> for F
    where
        F: FnMut(&T) -> Result<url::Url, E>,
    {
        type Error = E;
        fn get_url(&mut self, item: &T) -> Result<url::Url, Self::Error> {
            (self)(item)
        }
    }
}

mod path_url {
    use super::BeetItem;
    use crate::determined::UrlSource;

    // TODO add tests for Windows beet-path conversion to URL
    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    #[serde(from = "url::Url")]
    pub(super) struct BaseUrl(pub(super) url::Url);
    impl BaseUrl {
        // TODO delete if unused
        // pub fn new(url: &str) -> Result<Self, ErrorBase> {
        //     url.parse().map(Self).map_err(|error| ErrorBase {
        //         base_url_str: url.to_string(),
        //         error,
        //     })
        // }
    }
    impl From<url::Url> for BaseUrl {
        fn from(value: url::Url) -> Self {
            Self(value)
        }
    }

    impl UrlSource<BeetItem> for BaseUrl {
        type Error = ErrorBeetPath;

        fn get_url(&mut self, item: &BeetItem) -> Result<url::Url, ErrorBeetPath> {
            // SOURCE `url::parser::PATH` not public, and somehow not used for `url::Url::join`
            // <https://github.com/servo/rust-url/blob/7492360d4230b67fa0e62794b6fde276525e5f84/url/src/parser.rs#L23>
            const PATH: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
                .add(b' ')
                .add(b'"')
                .add(b'<')
                .add(b'>')
                .add(b'`')
                //
                .add(b'#')
                .add(b'?')
                .add(b'{')
                .add(b'}');

            let base_url = &self.0;

            let path = item.get_path();
            let path = path.strip_prefix('/').unwrap_or(path);
            let path_percentencoded = percent_encoding::utf8_percent_encode(path, PATH).to_string();
            let path = &path_percentencoded;

            let url = base_url.join(path).map_err(|error| ErrorBeetPath {
                item: item.clone(),
                error,
            })?;

            Ok(url)
        }
    }

    // TODO delete if unused
    // #[derive(Debug)]
    // pub(super) struct ErrorBase {
    //     base_url_str: String,
    //     error: url::ParseError,
    // }
    // impl std::error::Error for ErrorBase {
    //     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    //         Some(&self.error)
    //     }
    // }
    // impl std::fmt::Display for ErrorBase {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         let Self {
    //             base_url_str,
    //             error: _,
    //         } = self;
    //         write!(f, "invalid base URL {base_url_str:?}")
    //     }
    // }
    #[derive(Debug)]
    pub(super) struct ErrorBeetPath {
        item: BeetItem,
        error: url::ParseError,
    }
    impl std::error::Error for ErrorBeetPath {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.error)
        }
    }
    impl std::fmt::Display for ErrorBeetPath {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { item, error: _ } = self;
            write!(
                f,
                "invalid beet URL for id {id}: {url:?}",
                id = item.get_beet_id(),
                url = item.get_path(),
            )
        }
    }

    #[cfg(test)]
    mod tests {
        use super::BaseUrl;
        use crate::{determined::UrlSource as _, todo_move_to_a_beet_lib::BeetItem};

        #[test]
        fn beet_path_not_fragment() {
            for input in [
                "/path/to/file_containing_#_sign.txt",
                "/path/to/file that contains #hash tag signs and other symbols {},%$#%#$@#?!@",
            ] {
                let item = BeetItem::test_creation(0, input.to_string());
                let mut base = BaseUrl("file:///some/base/".parse().expect("test base url valid"));

                let result = base.get_url(&item).expect("test item url valid");
                assert_eq!(
                    result.fragment(),
                    None,
                    "should not have fragment for input: {input}"
                );
            }
        }
    }
}

fn setup_spigot() -> eyre::Result<bucket_spigot::Network<BeetItem, String>> {
    use bucket_spigot::{
        order::OrderType,
        path::{Path, PathRef},
        ModifyCmd, Network,
    };

    let mut spigot = Network::default();
    let init_commands = {
        let root: Path = ".".parse().expect("valid path .");
        let split: Path = ".0".parse().expect("valid path .0");
        let bucket1: Path = ".0.0".parse().expect("valid path .0.0");
        let bucket2: Path = ".0.1".parse().expect("valid path .0.1");

        // TODO: use the Network creation script... Luke!
        vec![
            ModifyCmd::AddJoint { parent: root },
            ModifyCmd::AddBucket {
                parent: split.clone(),
            },
            ModifyCmd::AddBucket { parent: split },
            ModifyCmd::SetFilters {
                path: bucket1.clone(),
                new_filters: [
                    //
                    "added:2020..",
                    "grouping::^$",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            },
            ModifyCmd::SetOrderType {
                path: bucket1,
                new_order_type: OrderType::Shuffle,
            },
            ModifyCmd::SetFilters {
                path: bucket2.clone(),
                new_filters: [
                    //
                    "grouping::1|2|3|4|5",
                    "has_lyrics::^$",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            },
            ModifyCmd::SetOrderType {
                path: bucket2,
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
        info!("fill bucket {bucket} with {} items", new_contents.len());
        spigot.modify(ModifyCmd::FillBucket {
            bucket,
            new_contents,
        })?;
    }

    Ok(spigot)
}

// TODO move to a beet lib, likely also with BeetItem.url -> url::Url logic as well (see `mod path_url`)
mod todo_move_to_a_beet_lib {
    pub use self::beet_item::BeetItem;
    use std::{borrow::Cow, io::BufRead, process::Command};
    use tracing::{debug, trace};

    pub(super) fn query_beet(
        filters: impl Iterator<Item = String>,
    ) -> Result<Vec<BeetItem>, Error> {
        let make_error = |kind| Error { kind };

        debug!("spawn `beet` command");

        let mut command = Command::new("beet");
        command
            //
            .arg("ls")
            .arg("-f")
            .arg("$id=$path")
            .args(filters);

        trace!(?command);

        let output = command
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

        debug!("parse `beet` output ({} bytes)", output.stdout.len());

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
            #[cfg(test)]
            pub(crate) fn test_creation(beet_id: u64, path: String) -> Self {
                Self { beet_id, path }
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
