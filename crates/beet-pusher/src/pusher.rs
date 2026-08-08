// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
pub use self::fill_determined::FillError as FillDeterminedError;
pub use self::push_playlist::HintNeedPlaylistUpdate;
use crate::{BaseUrl, BeetItem, Determined};
use bucket_spigot::{Network, order::ArbitrarySource};

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

/// Sends [`bucket_spigot::Network`] items to VLC
///
/// NOTE: Requires an external [`vlc_http::ClientState`] for some action. This
/// helps with separating the main sequencing logic (queued items) from the I/O
/// (cached VLC state)
pub struct BeetPusher<'a, R> {
    spigot: bucket_spigot::Network<BeetItem, String>,
    rng: &'a mut R,
    // client_state: vlc_http::ClientState,
    determined: Determined<BeetItem>,
    config: Config,
}
struct Config {
    base_url: BaseUrl,
}

/// VLC client state with the [`vlc_http::sync::EndpointRequestor`], for use in the HTTP control
/// actions of [`BeetPusher`]
pub struct VlcDriver<T> {
    pub(crate) client_state: vlc_http::ClientState,
    pub(crate) http_runner: T,
}
impl<T> VlcDriver<T> {
    /// Wraps the components together
    pub fn new(client_state: vlc_http::ClientState, http_runner: T) -> Self {
        Self {
            client_state,
            http_runner,
        }
    }
    /// Combines the specified runner with the default [`vlc_http::ClientState`]
    pub fn default_from_runner(http_runner: T) -> Self {
        Self::new(vlc_http::ClientState::new(), http_runner)
    }
    /// Destructures into the inner parts
    pub fn into_parts(self) -> (vlc_http::ClientState, T) {
        let Self {
            client_state,
            http_runner,
        } = self;
        (client_state, http_runner)
    }
}

impl<'a, R> BeetPusher<'a, R>
where
    R: ArbitrarySource,
{
    /// Creates a new VLC client fed by the specified Network
    pub fn new(rng: &'a mut R, spigot: Network<BeetItem, String>, base_url: BaseUrl) -> Self {
        Self {
            spigot,
            rng,
            determined: Determined::default(),
            config: Config { base_url },
        }
    }
}
impl<R> BeetPusher<'_, R> {
    /// Allows mutation of the inner [`Network`]
    pub fn get_spigot_mut(&mut self) -> &mut Network<BeetItem, String> {
        &mut self.spigot
    }
    /// Returns `true` if VLC has consumed all determined items (i.e. a refill is needed)
    #[must_use]
    pub fn is_determined_empty(&self) -> bool {
        self.determined.is_empty()
    }
}

mod sync {
    use crate::pusher::VlcDriver;

    // type UreqError = vlc_http::http_runner::ureq::Error;
    pub(super) type ExhaustResult<'a, T, E> =
        Result<<T as vlc_http::Plan>::Output<'a>, vlc_http::sync::Error<T, E>>;

    impl<T> VlcDriver<T>
    where
        T: vlc_http::sync::EndpointRequestor,
    {
        /// Thin wrapper around [`vlc_http::sync::complete_plan`] with sane defaults
        ///
        /// # Errors
        /// Returns an error if the plan execution requests fail, see the function linked above for
        /// details
        pub(crate) fn complete_plan<U>(&mut self, query: U) -> ExhaustResult<'_, U, T::Error>
        where
            U: vlc_http::Plan,
            T::Error: std::error::Error + 'static,
        {
            const MAX_ENDPOINTS_PER_ACTION: usize = 100;

            let output = vlc_http::sync::complete_plan(
                query,
                &mut self.client_state,
                &mut self.http_runner,
                MAX_ENDPOINTS_PER_ACTION,
            )?;
            Ok(output)
        }
    }
}

mod fill_determined {
    use super::BeetPusher;
    use crate::BeetItem;
    use bucket_spigot::order::ArbitrarySource;

    impl<R: ArbitrarySource> BeetPusher<'_, R> {
        /// Updates the determined playlist items
        ///
        /// # Errors
        /// Returns an error if the beet path is invalid or the [`ArbitrarySource`] fails
        ///
        /// # Panics
        /// Panics if the determined logic does not yield 1 item (TODO!!!)
        pub(super) fn fill_determined(&mut self) -> Result<(), FillError<R::Error>> {
            tracing::debug!("fill determined...");

            if self.spigot.is_empty() {
                // nothing to do, no source data
                return Ok(());
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
                    .map_err(FillError::Arbitrary)?;

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
                    .map_err(FillError::BeetPath)?;

                // TODO: can this "finalize" be deferred?
                // e.g. store the `PeekAccepted` in self, and finalize only when
                // the item enters VLC (maximum user control of "next")
                // - likely need a wrapper around `spigot` to clarify "accept" vs
                //   "destroy" actions for each access
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
    }

    /// Error filling the determined items list (from the bucket spigot)
    #[derive(Debug, thiserror::Error)]
    pub enum FillError<E> {
        /// Fill failed to get randomness
        #[error("failed to get randomness")]
        Arbitrary(#[source] E),
        /// Fill failed to convert beet paths
        #[error("failed to convert beet paths")]
        BeetPath(#[source] crate::path_url::ErrorBeetPath),
    }
}

mod push_playlist {
    use crate::{FillDeterminedError, pipe_exec::VlcCmd, pusher::VlcDriver};

    use super::{BeetPusher, NowPlayingObserver};
    use bucket_spigot::order::ArbitrarySource;
    use vlc_http::{
        ClientState,
        goal::{ActionQuerySetItems, TargetPlaylistItems},
    };

    /// [`BeetPusher::push_playlist_update`] anticipates that it needs to run again
    pub enum HintNeedPlaylistUpdate {
        /// Available to run immediately (deterministic internal structure change)
        Immediate,
        /// Recommend to run after ~500ms, to allow VLC to catch up to reporting the new status
        WaitForVlc,
    }
    type PushPlaylistUpdateResult<T, U, R> = Result<Option<HintNeedPlaylistUpdate>, Error<T, U, R>>;

    impl<R: ArbitrarySource> BeetPusher<'_, R> {
        /// Pushes the determined track list to VLC, notifies the `now_playing_observer`
        /// for the current track if it changed, and refills the determined list if needed
        ///
        /// # Errors
        /// Returns an error if updating the determined list fails, or the [`NowPlayingObserver`]
        /// fails
        ///
        /// # See Also
        ///
        /// 1. [`BeetPusher::get_playlist_update_action`]
        /// 2. [`VlcDriver::run_playlist_update_action`]
        /// 3. [`BeetPusher::push_playlist_update_action`]
        pub fn push_playlist_update<T, U>(
            &mut self,
            vlc_driver: &mut VlcDriver<U>,
            now_playing_observer: Option<&mut T>,
        ) -> PushPlaylistUpdateResult<T::Error, U::Error, R::Error>
        where
            T: NowPlayingObserver,
            U: vlc_http::sync::EndpointRequestor,
            U::Error: std::error::Error + 'static,
        {
            let action_opt = self.get_playlist_update_action()?;

            let Some(action) = action_opt else {
                return Ok(None);
            };

            let playlist_update_counts = vlc_driver.run_playlist_update_action(action);

            self.push_playlist_update_action(playlist_update_counts, now_playing_observer)
        }
        /// Prepares the action required to push the determined track list to VLC
        ///
        /// # Errors
        /// Returns an error if updating the determined list fails, or the [`NowPlayingObserver`]
        /// fails
        ///
        /// # See Also
        ///
        /// - [`BeetPusher::push_playlist_update`]
        pub fn get_playlist_update_action<T, E>(
            &mut self,
        ) -> Result<Option<TargetPlaylistItems>, Error<T, E, R::Error>> {
            let make_err = |kind| Error { kind };

            self.fill_determined()
                .map_err(ErrorKind::FillDetermined)
                .map_err(make_err)?;

            if self.determined.is_empty() {
                // nothing to do, don't waste querying effort until we have items to push
                return Ok(None);
            }

            let target = TargetPlaylistItems::new()
                .set_urls(self.determined.urls().to_vec()) // FIXME cloning to vec feels so wrong...
                .set_keep_history(5);

            Ok(Some(target))
        }
    }
    /// Result from [`BeetPusher::run_playlist_update_action`]
    pub struct PlaylistUpdateCounts {
        vlc_len: usize,
        items_enqueued_count: usize,
    }
    impl<T> VlcDriver<T> {
        /// Executes the playlist update action on the VLC http endpoint
        ///
        /// # Errors
        ///
        /// Returns an error if the VLC operation(s) fail
        ///
        /// # See Also
        ///
        /// - [`BeetPusher::push_playlist_update`]
        pub fn run_playlist_update_action(
            &mut self,
            target: TargetPlaylistItems,
        ) -> Result<PlaylistUpdateCounts, Box<vlc_http::sync::Error<ActionQuerySetItems, T::Error>>>
        where
            T: vlc_http::sync::EndpointRequestor,
            T::Error: std::error::Error + 'static,
        {
            let action = self
                .client_state
                .build_plan()
                .set_playlist_and_query_matched(target);

            let playlist_result = self.complete_plan(action).map_err(Box::new)?;
            let vlc_len = playlist_result.get_matched_items().len();
            let items_enqueued_count = playlist_result.get_items_enqueued_count();
            Ok(PlaylistUpdateCounts {
                vlc_len,
                items_enqueued_count,
            })
        }
    }
    impl<R> BeetPusher<'_, R> {
        /// Updates the internal "determined" state based on the result from
        /// [`VlcDriver::run_playlist_update_action`], returning a hint whether
        /// more playlist items are needed from the spigot
        ///
        /// # Errors
        ///
        /// Returns an error if the playlist update failed, or resolving beet items fails
        ///
        /// # See Also
        ///
        /// - [`BeetPusher::push_playlist_update`]
        pub fn push_playlist_update_action<T, E, V>(
            &mut self,
            playlist_update_result: Result<
                PlaylistUpdateCounts,
                Box<vlc_http::sync::Error<ActionQuerySetItems, E>>,
            >,
            now_playing_observer: Option<&mut T>,
        ) -> Result<Option<HintNeedPlaylistUpdate>, Error<T::Error, E, V>>
        where
            T: NowPlayingObserver,
            E: std::error::Error + 'static,
        {
            let make_err = |kind| Error { kind };

            let PlaylistUpdateCounts {
                vlc_len,
                items_enqueued_count,
            } = playlist_update_result
                .map_err(ErrorKind::HttpRunner)
                .map_err(make_err)?;

            // remove completed items for the beginning of the `determined` list
            if let Some(excess_at_start) = self.determined.len().checked_sub(vlc_len) {
                let () = self
                    .determined
                    .modify(&self.config.base_url, |determined| {
                        let removed = determined.splice(0..excess_at_start, std::iter::empty());
                        if let Some(observer) = now_playing_observer {
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

            let hint = {
                // if VLC consumed the determined item, hint to immediately peek the next one
                // (faster than waiting for the next deferred cycle trigger)
                (self.is_determined_empty() && !self.spigot.is_empty())
                    .then_some(HintNeedPlaylistUpdate::Immediate)
            }
            .or_else(|| {
                // if queued any items, recommend repeating after VLC updates
                (items_enqueued_count > 0).then_some(HintNeedPlaylistUpdate::WaitForVlc)
            });
            Ok(hint)
        }
    }
    impl<R> BeetPusher<'_, R> {
        /// Runs the [`VlcCmd`] and updates the client state with the response
        ///
        /// # Errors
        /// Returns an error if requesting the command endpoint or parsing the
        /// response fails
        pub fn vlc_cmd<E>(
            client_state: &mut ClientState,
            http_runner: &mut impl vlc_http::sync::EndpointRequestor<Error = E>,
            cmd: VlcCmd,
        ) -> Result<(), E> {
            let cmd = vlc_http::Command::from(cmd);

            let response = http_runner.request(cmd.into())?;
            client_state.update(response);

            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct Error<E, F, G> {
        kind: ErrorKind<E, F, G>,
    }
    #[derive(Debug)]
    enum ErrorKind<E, F, G> {
        HttpRunner(Box<vlc_http::sync::Error<vlc_http::goal::ActionQuerySetItems, F>>),
        BeetPath(crate::path_url::ErrorBeetPath),
        Observer(E),
        FillDetermined(FillDeterminedError<G>),
    }
    impl<E, F, G> std::error::Error for Error<E, F, G>
    where
        E: std::error::Error + 'static,
        F: std::error::Error + 'static,
        G: std::error::Error + 'static,
    {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { kind } = self;
            match kind {
                ErrorKind::HttpRunner(source) => Some(source),
                ErrorKind::Observer(source) => Some(source),
                ErrorKind::BeetPath(source) => Some(source),
                ErrorKind::FillDetermined(source) => Some(source),
            }
        }
    }
    impl<E, F, G> std::fmt::Display for Error<E, F, G> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::HttpRunner(_) => write!(f, "failed to execute vlc_http set action"),
                ErrorKind::Observer(_) => write!(f, "failed to update the NowPlayingObserver"),
                ErrorKind::BeetPath(_) => {
                    write!(f, "failed to update the determined playlist URLs")
                }
                ErrorKind::FillDetermined(_) => write!(f, "failed to fill determined list"),
            }
        }
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
            // client_state,
            determined,
            config: Config { base_url },
        } = self;
        f.debug_struct("BeetPusher")
            .field("spigot", &DebugAsDisplay(spigot.view_table_default()))
            // .field("client_state", client_state)
            .field("determined.items", &determined.items())
            .field("determined.urls", &determined.urls())
            .field("config.base_url", base_url)
            .finish()
    }
}
