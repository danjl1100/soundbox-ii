// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Injects imperfections between the `vlc-http` logic and the simulated VLC instance

use self::alphanum_string::AlphanumString;
use self::arb_repeat_mode::ArbRepeatMode;
use self::ascii_string::AsciiString;
use self::model_endpoint_caller::ModelEndpointCaller;

use eyre::Context as _;
use std::{collections::VecDeque, str::FromStr, sync::LazyLock};
use tracing::{debug, info};
use url::Url;
use vlc_http::{Change, ClientState, goal::TargetPlaylistItems};

mod alphanum_string;
mod ascii_string;

mod arb_repeat_mode;

/// Arbitrary high level [`Change`] to apply to VLC
#[derive(Clone, Debug, arbitrary::Arbitrary)]
enum ArbChange {
    PlaybackMode {
        repeat: ArbRepeatMode,
        is_random: bool,
    },
    PlaylistSet {
        items: Vec<
            AlphanumString, // AsciiString
        >,
    },
}

impl ArbChange {
    /// Measure the expected number of steps for the change
    fn get_complexity(&self, current_len: usize) -> usize {
        match self {
            ArbChange::PlaybackMode {
                repeat: _,
                is_random: _,
            } => 4,
            ArbChange::PlaylistSet { items } => 2 * items.len() + current_len + 4,
        }
    }
}

impl From<ArbChange> for Change {
    fn from(value: ArbChange) -> Self {
        match value {
            ArbChange::PlaybackMode { repeat, is_random } => vlc_http::goal::PlaybackMode::new()
                .set_repeat(repeat.into())
                .set_random(is_random)
                .into(),
            ArbChange::PlaylistSet { items } => {
                let items = items
                    .into_iter()
                    .map(|s| Url::from_str(&format!("file:///{s}")).expect("valid URL"))
                    .collect();
                TargetPlaylistItems::new().set_urls(items).into()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, arbitrary::Arbitrary)]
enum Glitch {
    DropRequest,
    DelayRequest,
}
impl Glitch {
    /// Returns how much complexity this glitch adds for the specified [`Change`]
    fn get_added_complexity(
        self,
        change: &ArbChange,
        // current_items_len: usize,
    ) -> usize {
        match self {
            Glitch::DropRequest => 1,
            Glitch::DelayRequest => {
                #[expect(clippy::match_same_arms)]
                match change {
                    // delay is likely to repeat actions
                    ArbChange::PlaybackMode { .. } => 2,
                    // includes playback mode, above
                    ArbChange::PlaylistSet { items: _ } => 2,
                }
            }
        }
    }
}

enum GlitchSource {
    Once(VecDeque<Option<Glitch>>),
    // TODO
    // Repeat(Vec<Option<Glitch>>),
}
impl GlitchSource {
    pub fn empty() -> Self {
        Self::once(vec![])
    }
    pub fn once(value: Vec<Option<Glitch>>) -> Self {
        Self::Once(value.into())
    }

    /// Returns the how much complexity is added by the run of [`Glitch`]es for the specified
    /// [`Change`]
    fn get_added_complexity(
        &self,
        change: &ArbChange,
        // current_items_len: usize,
    ) -> usize {
        match self {
            GlitchSource::Once(glitches) => glitches
                .iter()
                .copied()
                .filter_map(|opt| Some(opt?.get_added_complexity(change)))
                .sum(),
        }
    }
    // TODO
    // pub fn repeat(pattern: Vec<Option<Glitch>>) -> Self {
    //     Self::Repeat(pattern)
    // }
}
impl Default for GlitchSource {
    fn default() -> Self {
        Self::empty()
    }
}
impl Iterator for GlitchSource {
    type Item = Option<Glitch>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            GlitchSource::Once(glitches) => glitches.pop_front(),
        }
    }
}

mod model_endpoint_caller {
    use super::{Glitch, GlitchSource};
    use std::str::FromStr as _;
    use vlc_http::{
        Endpoint,
        sync::EndpointRequestor,
        testing::{Model, ModelResponse},
    };

    /// Applies [`Endpoint`] request to a [`Model`], with optional interference from a determined
    /// [`GlitchSource`]
    #[derive(Default)]
    pub struct ModelEndpointCaller {
        model: Model,
        glitch_source: GlitchSource,
        /// State for [`Glitch::DelayRequest`]
        delayed_endpoint: Option<Endpoint>,
    }
    impl ModelEndpointCaller {
        /// Creates the [`Model`] with no [`Glitch`]es present
        pub fn new() -> Self {
            Self::default()
        }
        pub fn get_model(&self) -> &Model {
            &self.model
        }

        /// Sets the list of determined [`Glitch`]es
        pub fn replace_glitch_source(&mut self, glitch_source: GlitchSource) {
            self.glitch_source = glitch_source;
        }
    }
    impl EndpointRequestor for ModelEndpointCaller {
        type Error = Error;

        fn request(&mut self, endpoint: Endpoint) -> Result<vlc_http::Response, Self::Error> {
            let Self {
                model,
                glitch_source,
                delayed_endpoint,
            } = self;

            let make_err = |kind| Error { kind };

            if let Some(old) = delayed_endpoint.take() {
                let _hidden_response = model
                    .request(old.get_path_and_query())
                    .map_err(|source| ErrorKind::Request {
                        source,
                        endpoint: old,
                    })
                    .map_err(make_err)?;
            };

            let request = if let Some(glitch) = glitch_source.next().flatten() {
                const PATH_STATUS_JSON: &str = "/requests/status.json";
                const PATH_PLAYLIST_JSON: &str = "/requests/playlist.json";

                let path_and_query = endpoint.get_path_and_query();
                let Some(request_base_path) = [PATH_PLAYLIST_JSON, PATH_STATUS_JSON]
                    .into_iter()
                    .find(|path| path_and_query.starts_with(path))
                else {
                    panic!("unrecognized base path {path_and_query:?}");
                };

                match glitch {
                    Glitch::DropRequest => {
                        // respond correctly, but do not apply the command
                        request_base_path
                    }
                    Glitch::DelayRequest => {
                        // queue the currnet endpoint to apply next cycle
                        assert_eq!(
                            delayed_endpoint.replace(endpoint.clone()),
                            None,
                            "delayed_endpoint must be applied/emptied already"
                        );
                        // respond with the status (before the delayed command runs)
                        request_base_path
                    }
                }
            } else {
                // execute as-is
                endpoint.get_path_and_query()
            };

            let response = model
                .request(request)
                .map_err(|source| ErrorKind::Request { source, endpoint })
                .map_err(make_err)?;
            let response = match &response {
                vlc_http::testing::ModelResponse::Json(s) => vlc_http::Response::from_str(s)
                    .map_err(|source| ErrorKind::ResponseJson { source, response })
                    .map_err(make_err)?,
                vlc_http::testing::ModelResponse::Art => {
                    return Err(make_err(ErrorKind::ResponseArt));
                }
            };
            Ok(response)
        }
    }

    #[derive(Debug)]
    pub struct Error {
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Request {
            source: vlc_http::testing::RequestError,
            endpoint: Endpoint,
        },
        ResponseJson {
            source: vlc_http::response::ParseError,
            response: ModelResponse,
        },
        ResponseArt,
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { kind } = self;
            match kind {
                ErrorKind::Request { source, .. } => Some(source),
                ErrorKind::ResponseJson { source, .. } => Some(source),
                ErrorKind::ResponseArt => None,
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::Request {
                    source: _,
                    endpoint,
                } => write!(f, "model rejected request for {endpoint:?}"),
                ErrorKind::ResponseJson {
                    source: _,
                    response,
                } => {
                    write!(f, "invalid JSON response from model: {response:?}")
                }
                ErrorKind::ResponseArt => todo!(),
            }
        }
    }
}

/// List of changes to apply to VLC, with extra metadata `T` for each change
#[derive(Clone, Debug, arbitrary::Arbitrary)]
struct ArbChangesList<T> {
    changes: Vec<(ArbChange, T)>,
}

impl<T> Default for ArbChangesList<T> {
    fn default() -> Self {
        Self { changes: vec![] }
    }
}
impl<T> ArbChangesList<T> {
    fn push_glitches(&mut self, change: ArbChange, glitches: T) {
        let Self { changes } = self;
        changes.push((change, glitches));
    }
    fn map_inner<U>(self, map_fn: impl Fn(T) -> U) -> ArbChangesList<U> {
        let Self { changes } = self;
        let changes = changes
            .into_iter()
            .map(|(change, elem)| (change, map_fn(elem)))
            .collect();
        ArbChangesList { changes }
    }
}
impl ArbChangesList<Once<Glitches>> {
    fn push(&mut self, change: ArbChange) {
        self.push_glitches(change, Once(Glitches(vec![])));
    }
}
// impl ArbChangesList<NoGlitches> {
//     fn push(&mut self, change: ArbChange) {
//         self.push_glitches(change, NoGlitches);
//     }
// }

impl<T> ArbChangesList<T>
where
    T: Into<GlitchSource> + std::fmt::Debug,
{
    fn run_changes_list(self) -> eyre::Result<()> {
        let mut client_state = ClientState::new();
        let mut endpoint_caller = ModelEndpointCaller::new();

        eprintln!("{:-<80}", "");
        info!(?self);

        let ArbChangesList { changes } = self;
        for (change, glitches) in changes {
            let current_len = endpoint_caller.get_model().get_items().len();
            let change_complexity = change.get_complexity(current_len);

            debug!(?change);
            debug!(?glitches);

            let glitches = glitches.into();
            let glitch_complexity = glitches.get_added_complexity(
                &change,
                // current_len,
            );

            let max_iter_count = change_complexity + glitch_complexity;
            debug!(max_iter_count);

            let change = Change::from(change);

            dbg!((&change, change_complexity, glitch_complexity));

            let plan = client_state.build_plan().apply(change.clone());

            endpoint_caller.replace_glitch_source(glitches);

            vlc_http::sync::complete_plan(
                plan,
                &mut client_state,
                &mut endpoint_caller,
                max_iter_count,
            )
            .with_context(|| format!("failed to complete plan for {change:?}"))?;
        }

        Ok(())
    }
}

#[derive(Debug, arbitrary::Arbitrary)]
struct NoGlitches;
impl From<NoGlitches> for GlitchSource {
    fn from(_: NoGlitches) -> Self {
        GlitchSource::empty()
    }
}

#[derive(Clone, arbitrary::Arbitrary)]
struct Glitches(Vec<Option<Glitch>>);
impl std::fmt::Debug for Glitches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;

        let mut first = Some(());
        write!(f, "[")?;
        for elem in inner {
            if first.take().is_none() {
                write!(f, ", ")?;
            }
            let short = match elem {
                Some(Glitch::DropRequest) => "Drop",
                Some(Glitch::DelayRequest) => "Delay",
                None => "_",
            };
            write!(f, "{short}")?;
        }
        write!(f, "]")
    }
}
impl FromStr for Glitches {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(", ")
            .map(|part| {
                let elem = match part {
                    "_" => None,
                    "Delay" => Some(Glitch::DelayRequest),
                    "Drop" => Some(Glitch::DropRequest),
                    unknown => return Err(format!("unknown part {unknown:?}")),
                };
                Ok(elem)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
struct Once<T>(T);
impl From<Once<Glitches>> for GlitchSource {
    fn from(value: Once<Glitches>) -> Self {
        let Once(Glitches(inner)) = value;
        GlitchSource::once(inner)
    }
}

#[test]
fn arb_commands_perfect() {
    arbtest::arbtest(|u| {
        u.arbitrary::<ArbChangesList<NoGlitches>>()?
            .run_changes_list()
            .expect("changes should pass with no glitches");
        Ok(())
    });
}

#[test]
fn arb_commands_glitches() {
    arbtest::arbtest(|u| {
        let list = u.arbitrary::<ArbChangesList<Once<Glitches>>>()?;

        list.clone()
            .map_inner(|_| NoGlitches)
            .run_changes_list()
            .expect("changes should pass with no glitches");

        // run with the full glitches list
        list.run_changes_list()
            .expect("changes should pass WITH glitches too");

        Ok(())
    })
    // .seed(0x34dc7a9a00010000)
    ;
}

fn init_tracing() {
    static ONCE: LazyLock<()> = LazyLock::new(|| {
        use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().compact())
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    });
    *ONCE
}

#[test]
fn playlist_set_from_wrong_state() -> eyre::Result<()> {
    use ArbChange::{PlaybackMode, PlaylistSet};
    use ArbRepeatMode::All;

    init_tracing();

    let mut list = ArbChangesList::<Once<Glitches>>::default();
    list.push(PlaybackMode {
        repeat: All,
        is_random: true,
    });
    list.push(PlaylistSet { items: vec![] });

    list.run_changes_list()
}
#[test]
fn commands_glitches_case() -> eyre::Result<()> {
    use ArbChange::{PlaybackMode, PlaylistSet};
    use ArbRepeatMode::{All, One};

    init_tracing();

    let mut list = ArbChangesList::<Once<Glitches>>::default();
    list.push(PlaybackMode {
        repeat: One,
        is_random: true,
    });
    list.push_glitches(
        PlaybackMode {
            repeat: All,
            is_random: true,
        },
        Once("_, Delay, Drop, Delay, Delay".parse().unwrap()),
    );
    list.push(PlaylistSet { items: vec![] });

    list.run_changes_list()
}
