use crate::{BaseUrl, BeetItem, Determined};
use bucket_spigot::Network;

/// Listener for the current playing item
pub trait NowPlayingObserver {
    /// Error to propagate to the main application
    type Error: std::error::Error + Send + Sync + 'static;
    /// Notifies that VLC changed to playing the specified [`BeetItem`]
    ///
    /// # Errors
    /// Returns an error to propagate to the caller of [`BeetPusher`] methods
    fn now_playing(&mut self, item: &BeetItem) -> Result<(), Self::Error>;
}
impl<F, E> NowPlayingObserver for F
where
    F: FnMut(&BeetItem) -> Result<(), E>,
    E: std::error::Error + Send + Sync + 'static,
{
    type Error = E;
    fn now_playing(&mut self, item: &BeetItem) -> Result<(), Self::Error> {
        self(item)
    }
}

pub enum NoObserver {}
impl NowPlayingObserver for NoObserver {
    type Error = std::convert::Infallible;

    fn now_playing(&mut self, _: &BeetItem) -> Result<(), Self::Error> {
        match *self {}
    }
}

/// Sends [`bucket_spigot::Network`] items to VLC
pub struct BeetPusher<'a, R, T = NoObserver> {
    spigot: bucket_spigot::Network<BeetItem, String>,
    rng: &'a mut R,
    client_state: vlc_http::ClientState,
    // client: Client,
    // http_runner: vlc_http::http_runner::ureq::HttpRunner,
    determined: Determined<BeetItem>,
    config: Config,
    now_playing_observer: Option<T>,
}
// struct Client {
//     state: vlc_http::ClientState,
// }
// impl Client {
//     fn new() -> Self {
//         let state = vlc_http::ClientState::new();
//         Self { state }
//     }
// }
struct Config {
    base_url: BaseUrl,
}
impl<'a, R> BeetPusher<'a, R, NoObserver>
where
    R: rand::RngCore,
{
    /// Creates a new VLC client fed by the specified Network
    pub fn new(
        // auth: Auth,
        rng: &'a mut R,
        spigot: Network<BeetItem, String>,
        base_url: BaseUrl,
    ) -> Self {
        // let http_runner = vlc_http::http_runner::ureq::HttpRunner::new(auth);

        Self {
            spigot,
            rng,
            client_state: vlc_http::ClientState::default(),
            // client: Client::new(),
            // http_runner,
            determined: Determined::default(),
            config: Config { base_url },
            now_playing_observer: None,
        }
    }
}
impl<'a, R, T> BeetPusher<'a, R, T> {
    /// Replaces the [`NowPlayingObserver`]
    pub fn set_now_playing_observer<U>(self, now_playing_observer: U) -> BeetPusher<'a, R, U>
    where
        U: NowPlayingObserver,
    {
        let Self {
            spigot,
            rng,
            client_state,
            // client,
            // http_runner,
            determined,
            config,
            now_playing_observer: _, // replace
        } = self;
        BeetPusher {
            spigot,
            rng,
            client_state,
            // client,
            // http_runner,
            determined,
            config,
            now_playing_observer: Some(now_playing_observer),
        }
    }
    /// Allows mutation of the inner [`Network`]
    pub fn get_spigot_mut(&mut self) -> &mut Network<BeetItem, String> {
        &mut self.spigot
    }
}

mod sync {
    use super::BeetPusher;

    // type UreqError = vlc_http::http_runner::ureq::Error;
    type ExhaustResult<'a, T, E> =
        Result<<T as vlc_http::Plan>::Output<'a>, vlc_http::sync::Error<T, E>>;

    impl<R, T> BeetPusher<'_, R, T>
    where
        R: rand::RngCore,
    {
        /// Thin wrapper around [`vlc_http::sync::complete_plan`] with sane defaults
        ///
        /// # Errors
        /// Returns an error if the plan execution requests fail, see the function linked above for
        /// details
        pub(crate) fn complete_plan<'a, U, E>(
            query: U,
            client_state: &'a mut vlc_http::ClientState,
            http_runner: &mut impl vlc_http::sync::EndpointRequestor<Error = E>,
        ) -> ExhaustResult<'a, U, E>
        where
            U: vlc_http::Plan,
            E: std::error::Error + 'static,
        {
            const MAX_ENDPOINTS_PER_ACTION: usize = 100;
            let output = vlc_http::sync::complete_plan(
                query,
                client_state,
                http_runner,
                MAX_ENDPOINTS_PER_ACTION,
            )?;
            Ok(output)
        }
    }
}

mod fill_determined {
    use super::BeetPusher;
    use crate::BeetItem;
    use tracing::debug;

    impl<R: rand::RngCore, T> BeetPusher<'_, R, T> {
        /// Updates the determined playlist items
        ///
        /// # Errors
        /// Returns an error if the spigot is empty
        ///
        /// # Panics
        /// Panics if the determined logic does not yield 1 item (TODO!!!)
        pub fn fill_determined(&mut self) -> Result<(), Error> {
            use ErrorKind;
            let make_err = |kind| Error { kind };

            if self.spigot.is_empty() {
                let view = self.spigot.view_table_default();
                return Err(make_err(ErrorKind::SpigotEmptyError {
                    view: view.to_string(),
                }));
            }

            let peek_len = match self.determined.items().len() {
                len @ 0..=0 => Some(1 - len),
                1 => None,
                2.. => unreachable!("determined should be 1 item or fewer"),
            };

            if let Some(peek_len) = peek_len {
                let peeked = self
                    .spigot
                    .peek(self.rng, peek_len)
                    .map_err(ErrorKind::Rand)
                    .map_err(make_err)?;

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
                    .modify(&self.config.base_url, |dest: &mut Vec<BeetItem>| {
                        dest.extend(peeked.items().iter().map(|&item| item.clone()));
                    })
                    .map_err(ErrorKind::BeetPath)
                    .map_err(make_err)?;

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
    }

    #[derive(Debug)]
    pub struct Error {
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        SpigotEmptyError { view: String },
        Rand(rand::Error),
        BeetPath(crate::path_url::ErrorBeetPath),
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { kind } = self;
            match kind {
                ErrorKind::SpigotEmptyError { .. } => None,
                ErrorKind::Rand(source) => Some(source),
                ErrorKind::BeetPath(source) => Some(source),
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::SpigotEmptyError { view } => {
                    write!(f, "bucket-spigot network must be non-empty:\n{view}")
                }
                ErrorKind::Rand(_source) => write!(f, "failed to get randomness"),
                ErrorKind::BeetPath(_source) => write!(f, "failed to convert beet paths"),
            }
        }
    }
}

mod push_playlist {
    use super::{BeetPusher, NowPlayingObserver};
    use vlc_http::goal::TargetPlaylistItems;

    // TODO remove if unused
    // type InnerPlan = vlc_http::goal::ActionQuerySetItems;

    // /// Output returned from executing the plan from [`BeetPusher::get_playlist_update`]
    // #[derive(Clone, Copy, Debug)]
    // pub struct PlaylistUpdate<'a>(<InnerPlan as vlc_http::Plan>::Output<'a>);

    // #[derive(Debug)]
    // pub struct PlaylistUpdatePlan(InnerPlan);
    // impl vlc_http::Plan for PlaylistUpdatePlan {
    //     type Output<'a> = PlaylistUpdate<'a>;

    //     fn next<'a>(
    //         &mut self,
    //         state: &'a vlc_http::ClientState,
    //     ) -> Result<vlc_http::goal::Step<Self::Output<'a>>, vlc_http::goal::Error> {
    //         let Self(action) = self;
    //         action.next(state).map(|step| step.map(PlaylistUpdate))
    //     }
    // }

    impl<R: rand::RngCore, T> BeetPusher<'_, R, T>
    where
        T: NowPlayingObserver,
    {
        /// Pushes the determined track list to VLC and notifies the `now_playing_observer`
        /// for the current track if it changed
        ///
        /// # Errors
        /// Returns an error if updating the determined list fails, or the [`NowPlayingObserver`]
        /// fails (if any)
        pub fn push_playlist_update<E>(
            &mut self,
            http_runner: &mut impl vlc_http::sync::EndpointRequestor<Error = E>,
        ) -> Result<(), Error<T::Error, E>>
        where
            E: std::error::Error + 'static,
        {
            let make_err = |kind| Error { kind };

            let target = TargetPlaylistItems::new()
                .set_urls(self.determined.urls().to_vec()) // FIXME cloning to vec feels so wrong...
                .set_keep_history(5);

            let action = self
                .client_state
                .build_plan()
                .set_playlist_and_query_matched(target);

            let vlc_list = Self::complete_plan(action, &mut self.client_state, http_runner)
                .map_err(Box::new)
                .map_err(ErrorKind::HttpRunner)
                .map_err(make_err)?;
            let vlc_len = vlc_list.len();
            // remove completed items for the beginning of the `determined` list
            if let Some(excess_at_start) = self.determined.len().checked_sub(vlc_len) {
                let () = self
                    .determined
                    .modify(&self.config.base_url, |determined| {
                        let removed = determined.splice(0..excess_at_start, std::iter::empty());
                        if let Some(observer) = &mut self.now_playing_observer {
                            for item in removed {
                                // TODO only notify of only the last one?
                                //
                                // TODO what is the API contract of `now_playing_observer`
                                observer.now_playing(&item)?;
                            }
                        }
                        Ok::<_, T::Error>(())
                    })
                    .map_err(ErrorKind::BeetPath)
                    .map_err(make_err)?
                    .map_err(ErrorKind::Observer)
                    .map_err(make_err)?;
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct Error<E, F> {
        kind: ErrorKind<E, F>,
    }
    #[derive(Debug)]
    enum ErrorKind<E, F> {
        HttpRunner(Box<vlc_http::sync::Error<vlc_http::goal::ActionQuerySetItems, F>>),
        BeetPath(crate::path_url::ErrorBeetPath),
        Observer(E),
    }
    impl<E, F> std::error::Error for Error<E, F>
    where
        E: std::error::Error + 'static,
        F: std::error::Error + 'static,
    {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { kind } = self;
            match kind {
                ErrorKind::HttpRunner(source) => Some(source),
                ErrorKind::Observer(source) => Some(source),
                ErrorKind::BeetPath(source) => Some(source),
            }
        }
    }
    impl<E, F> std::fmt::Display for Error<E, F> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::HttpRunner(_) => write!(f, "failed to execute vlc_http set action"),
                ErrorKind::Observer(_) => write!(f, "failed to update the NowPlayingObserver"),
                ErrorKind::BeetPath(_) => {
                    write!(f, "failed to update the determined playlist URLs")
                }
            }
        }
    }
}

impl<R, T> std::fmt::Debug for BeetPusher<'_, R, T> {
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
            client_state,
            // client: Client { state },
            // http_runner: _,
            determined,
            config: Config { base_url },
            now_playing_observer: _,
        } = self;
        f.debug_struct("BeetPusher")
            .field("spigot", &DebugAsDisplay(spigot.view_table_default()))
            .field("client_state", client_state)
            .field("determined.items", &determined.items())
            .field("determined.urls", &determined.urls())
            .field("config.base_url", base_url)
            .finish()
    }
}
