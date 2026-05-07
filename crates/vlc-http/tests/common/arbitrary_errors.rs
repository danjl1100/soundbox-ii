// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Injects imperfections between the `vlc-http` logic and the simulated VLC instance

use vlc_http_test::arb_goal::{ArbGoal, ArbPlaybackMode, ArbTargetPlaylistItems};

use self::model_endpoint_caller::ModelEndpointCaller;
use vlc_http_test::arb_goal::ArbRepeatMode;

use eyre::Context as _;
use std::{collections::VecDeque, str::FromStr};
use tracing::{debug, info};
use vlc_http::{ClientState, Goal};

#[derive(Clone, Copy, Debug, arbitrary::Arbitrary)]
enum Glitch {
    DropRequest,
    DelayRequest,
}
impl Glitch {
    /// Returns how much complexity this glitch adds for the specified [`Goal`]
    fn get_added_complexity(
        self,
        goal: &ArbGoal,
        // current_items_len: usize,
    ) -> usize {
        match self {
            Glitch::DropRequest => 1,
            Glitch::DelayRequest => {
                #[expect(clippy::match_same_arms, reason = "clarify logic difference")]
                match goal {
                    // delay is likely to repeat actions
                    ArbGoal::PlaybackMode(ArbPlaybackMode { .. }) => 2,
                    // includes playback mode, above
                    ArbGoal::PlaylistSet(ArbTargetPlaylistItems { items: _ }) => 2,
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
    /// [`Goal`]
    fn get_added_complexity(
        &self,
        goal: &ArbGoal,
        // current_items_len: usize,
    ) -> usize {
        match self {
            GlitchSource::Once(glitches) => glitches
                .iter()
                .copied()
                .filter_map(|opt| Some(opt?.get_added_complexity(goal)))
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
    use vlc_http::{Endpoint, sync::EndpointRequestor};
    use vlc_http_test::model::{Model, ModelResponse};

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
            }

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
                vlc_http_test::model::ModelResponse::Json(s) => vlc_http::Response::from_str(s)
                    .map_err(|source| ErrorKind::ResponseJson { source, response })
                    .map_err(make_err)?,
                vlc_http_test::model::ModelResponse::Art => {
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
            source: vlc_http_test::model::RequestError,
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

/// List of goals to apply to VLC, with extra metadata `T` for each goals
#[derive(Clone, Debug, arbitrary::Arbitrary)]
struct ArbGoalsList<T> {
    goals: Vec<(ArbGoal, T)>,
}

impl<T> Default for ArbGoalsList<T> {
    fn default() -> Self {
        Self { goals: vec![] }
    }
}
impl<T> ArbGoalsList<T> {
    fn push_glitches(&mut self, goal: impl Into<ArbGoal>, glitches: T) {
        let Self { goals } = self;
        goals.push((goal.into(), glitches));
    }
    fn map_inner<U>(self, map_fn: impl Fn(T) -> U) -> ArbGoalsList<U> {
        let Self { goals } = self;
        let goals = goals
            .into_iter()
            .map(|(goal, elem)| (goal, map_fn(elem)))
            .collect();
        ArbGoalsList { goals }
    }
}
impl ArbGoalsList<Once<Glitches>> {
    fn push(&mut self, goal: impl Into<ArbGoal>) {
        self.push_glitches(goal.into(), Once(Glitches(vec![])));
    }
}
// impl ArbGoalsList<NoGlitches> {
//     fn push(&mut self, goal: ArbGoal) {
//         self.push_glitches(goal, NoGlitches);
//     }
// }

impl<T> ArbGoalsList<T>
where
    T: Into<GlitchSource> + std::fmt::Debug,
{
    fn run_goals_list(self) -> eyre::Result<()> {
        let mut client_state = ClientState::new();
        let mut endpoint_caller = ModelEndpointCaller::new();

        eprintln!("{:-<80}", "");
        info!(?self);

        let ArbGoalsList { goals } = self;
        for (goal, glitches) in goals {
            let current_len = endpoint_caller.get_model().get_items().len();
            let goal_complexity = goal.get_complexity(current_len);

            debug!(?goal);
            debug!(?glitches);

            let glitches = glitches.into();
            let glitch_complexity = glitches.get_added_complexity(
                &goal,
                // current_len,
            );

            let max_iter_count = goal_complexity + glitch_complexity;
            debug!(max_iter_count);

            let goal = Goal::from(goal);

            dbg!((&goal, goal_complexity, glitch_complexity));

            let plan = client_state.build_plan().apply(goal.clone());

            endpoint_caller.replace_glitch_source(glitches);

            vlc_http::sync::complete_plan(
                plan,
                &mut client_state,
                &mut endpoint_caller,
                max_iter_count,
            )
            .with_context(|| format!("failed to complete plan for {goal:?}"))?;
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
        u.arbitrary::<ArbGoalsList<NoGlitches>>()?
            .run_goals_list()
            .expect("goals should pass with no glitches");
        Ok(())
    });
}

#[test]
fn arb_commands_glitches() {
    arbtest::arbtest(|u| {
        let list = u.arbitrary::<ArbGoalsList<Once<Glitches>>>()?;

        list.clone()
            .map_inner(|_| NoGlitches)
            .run_goals_list()
            .expect("goals should pass with no glitches");

        // run with the full glitches list
        list.run_goals_list()
            .expect("goals should pass WITH glitches too");

        Ok(())
    })
    // .seed(0x34dc7a9a00010000)
    ;
}

fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().compact())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[test]
fn playlist_set_from_wrong_state() -> eyre::Result<()> {
    use ArbRepeatMode::All;

    init_tracing();

    let mut list = ArbGoalsList::<Once<Glitches>>::default();
    list.push(ArbPlaybackMode {
        repeat: All,
        is_random: true,
    });
    list.push(ArbTargetPlaylistItems { items: vec![] });

    list.run_goals_list()
}
#[test]
fn commands_glitches_case() -> eyre::Result<()> {
    use ArbRepeatMode::{All, One};

    init_tracing();

    let mut list = ArbGoalsList::<Once<Glitches>>::default();
    list.push(ArbPlaybackMode {
        repeat: One,
        is_random: true,
    });
    list.push_glitches(
        ArbPlaybackMode {
            repeat: All,
            is_random: true,
        },
        Once("_, Delay, Drop, Delay, Delay".parse().unwrap()),
    );
    list.push(ArbTargetPlaylistItems { items: vec![] });

    list.run_goals_list()
}
