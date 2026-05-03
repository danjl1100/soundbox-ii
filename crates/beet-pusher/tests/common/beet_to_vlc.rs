// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tests the logic from fake injected beet responses up to the VLC http requests

use self::expect_beet::ExpectBeet;
use self::expect_http::ExpectHttp;
use beet_pusher::{BeetItem, BeetPusher, NowPlayingObserver};
use bucket_spigot::order::ArbitrarySource;
use std::str::FromStr;
use vlc_http::testing::{Model, PlayState};

struct PanicRng;
impl ArbitrarySource for PanicRng {
    type Error = std::convert::Infallible;

    fn try_fill(&mut self, _dest: &mut [u8]) -> Result<(), Self::Error> {
        unimplemented!("PanicRng")
    }
}

mod expect_beet {
    use beet_pusher::BeetRunner;
    use std::{collections::VecDeque, process::ExitStatus};

    pub struct ExpectBeet {
        args_to_results: VecDeque<(&'static [&'static str], &'static str)>,
        // pub args_per_call: Vec<Vec<String>>,
    }
    impl ExpectBeet {
        pub fn new(args_to_results: &[(&'static [&'static str], &'static str)]) -> Self {
            let args_to_results = args_to_results.to_vec().into();
            Self { args_to_results }
        }
        pub fn assert_empty(self) {
            let Self { args_to_results } = self;
            assert_eq!(args_to_results, &[]);
        }
    }
    impl BeetRunner for ExpectBeet {
        type Error = std::convert::Infallible;

        fn run_beet_command<S>(
            &mut self,
            args: impl Iterator<Item = S>,
        ) -> Result<std::process::Output, Self::Error>
        where
            S: AsRef<str>,
        {
            let Self { args_to_results } = self;

            let found_args: Vec<String> = args.map(|s| s.as_ref().to_string()).collect();
            let Some((expected_args, stdout)) = args_to_results.pop_front() else {
                panic!("no request expected, but found: {found_args:?}")
            };
            assert_eq!(found_args, expected_args);
            // args_per_call.push(args);

            Ok(std::process::Output {
                status: ExitStatus::default(),
                stdout: stdout.bytes().collect(),
                stderr: vec![],
            })
        }
    }
    impl std::fmt::Debug for ExpectBeet {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ExpectBeet").finish_non_exhaustive()
        }
    }
}

mod expect_http {
    use vlc_http::{
        sync::EndpointRequestor,
        testing::{Model, ModelResponse},
    };

    pub struct ExpectHttp<'a> {
        model: &'a mut Model,
        requests: Vec<String>,
    }
    impl<'a> ExpectHttp<'a> {
        pub fn new(model: &'a mut Model) -> Self {
            Self {
                model,
                requests: vec![],
            }
        }
        // TODO remove if not wanting to test specifics
        // #[track_caller]
        // pub fn assert_requests(self, expected_requests: &[&str]) {
        //     let Self { model: _, requests } = self;
        //     assert_eq!(requests, expected_requests);
        // }
        #[track_caller]
        pub fn assert_some_contains(&self, needle: &str) {
            let Self { model: _, requests } = self;
            assert!(
                requests.iter().any(|s| s.contains(needle)),
                "expected some to contain {needle:?} in: {requests:?}"
            );
        }
        #[track_caller]
        pub fn assert_none_contains(&self, needle: &str) {
            let Self { model: _, requests } = self;
            assert!(
                requests.iter().all(|s| !s.contains(needle)),
                "expected none to contain {needle:?} in: {requests:?}"
            );
        }
        #[track_caller]
        pub fn assert_empty(&self) {
            let Self { model: _, requests } = self;
            assert!(
                requests.is_empty(),
                "expected no requests, found: {requests:?}"
            );
        }
    }
    impl EndpointRequestor for ExpectHttp<'_> {
        type Error = vlc_http::testing::RequestError;

        fn request(
            &mut self,
            endpoint: vlc_http::Endpoint,
        ) -> Result<vlc_http::Response, Self::Error> {
            let Self { model, requests } = self;

            let request = endpoint.get_path_and_query();
            requests.push(request.to_string());

            let response = model.request(request)?;
            let response = match response {
                ModelResponse::Json(response) => response,
                ModelResponse::Art => unimplemented!(),
            };
            let response = vlc_http::Response::from_slice(response.as_bytes())
                .expect("Model gives valid response");
            Ok(response)
        }
    }
}

struct TestBeetItems<'a>(std::borrow::Cow<'a, [BeetItem]>);
impl TestBeetItems<'_> {
    fn as_slice(&self) -> &[BeetItem] {
        &self.0
    }
}
impl<'a, const N: usize> From<&'a [BeetItem; N]> for TestBeetItems<'a> {
    fn from(value: &'a [BeetItem; N]) -> Self {
        (&value[..]).into()
    }
}
impl<'a> From<&'a [BeetItem]> for TestBeetItems<'a> {
    fn from(value: &'a [BeetItem]) -> Self {
        Self(value.into())
    }
}
impl FromStr for TestBeetItems<'static> {
    type Err = <BeetItem as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let items = s
            .lines()
            .map(str::parse)
            .collect::<Result<Vec<BeetItem>, _>>()?;
        Ok(Self(items.into()))
    }
}

#[derive(Debug, Default)]
pub struct NowPlaying {
    items: Vec<BeetItem>,
}
impl NowPlaying {
    #[track_caller]
    fn assert_playing<'a, T>(self, expected: T)
    where
        T: Into<TestBeetItems<'a>>,
    {
        let expected = expected.into();
        let expected = expected.as_slice();

        assert_eq!(self.items, expected);
    }
}
impl NowPlayingObserver for NowPlaying {
    type Error = std::convert::Infallible;

    fn now_playing(&mut self, item: &BeetItem) -> Result<(), Self::Error> {
        self.items.push(item.clone());
        Ok(())
    }
}

/// Returns a [`BeetPusher`] that panics if requesting randomization (useful for tests)
fn new_test_beet_pusher(
    spigot: bucket_spigot::Network<BeetItem, String>,
) -> BeetPusher<'static, PanicRng> {
    let base_url = beet_pusher::BaseUrl("file://host/".parse().expect("valid URL"));
    let rng = Box::leak(Box::new(PanicRng));

    BeetPusher::new(rng, spigot, base_url)
}

#[track_caller]
fn new_beet_spigot(script: &str) -> bucket_spigot::Network<BeetItem, String> {
    bucket_spigot::Network::<BeetItem, String>::from_commands_str_whitespace(script)
        .expect("valid script")
}

/// Must tolerate empty results, in case of transient invalid queries while the user is editing
#[test]
fn empty_beet_result() -> eyre::Result<()> {
    let spigot = new_beet_spigot("add-bucket .");

    let pusher = &mut new_test_beet_pusher(spigot);
    let model = &mut Model::default();

    {
        let mut runner = ExpectBeet::new(&[(&["ls", "-f$id=$path"], "")]);
        pusher.fill_buckets(&mut runner)?;
        runner.assert_empty();
    }

    let push_result = pusher.fill_determined();
    assert!(
        matches!(
            push_result,
            Err(beet_pusher::FillDeterminedError::SpigotEmptyError(_))
        ),
        "expected SpigotEmptyError, found {push_result:?}"
    );

    let (runner, now_playing) = push_playlist_update(pusher, model)?;
    now_playing.assert_playing(&[]);
    runner.assert_empty();

    Ok(())
}

#[test]
fn queries_beet_for_buckets() -> eyre::Result<()> {
    // pseudocode:
    // - spigot is one bucket
    // - verify calls beet with format arg
    // - given response is supplied, verify

    let spigot = new_beet_spigot(
        "
    add-bucket .
    ",
    );

    let beet_items_str = "1=first
2=second
3=third";

    let file_urls = [
        "file://host/first",
        "file://host/second",
        "file://host/third",
    ];

    let pusher = &mut new_test_beet_pusher(spigot);
    let model = &mut Model::default();

    {
        let mut runner = ExpectBeet::new(&[(&["ls", "-f$id=$path"], beet_items_str)]);
        pusher.fill_buckets(&mut runner)?;
        runner.assert_empty();
    }

    let beet_items: TestBeetItems = beet_items_str.parse().unwrap();
    let beet_items = beet_items.as_slice();

    for (loop_index, file_url) in file_urls.iter().copied().enumerate() {
        let file_url_encoded = urlencoding::encode(file_url).to_string();

        let expected_beet_items = &beet_items[loop_index..=loop_index];

        pusher.fill_determined()?;

        {
            let (runner, now_playing) = push_playlist_update(pusher, model)?;
            now_playing.assert_playing(&[]);
            runner.assert_some_contains(&file_url_encoded);
            // TODO remove if not wanting to test specifics
            // runner.assert_requests(&[
            //     "/requests/status.json",
            //     "/requests/playlist.json",
            //     &format!("/requests/playlist.json?command=in_enqueue&input={file_url_encoded}"),
            // ]);
        }

        let items = model.get_items();
        let Some(set_playing_id) = items
            .iter()
            .find_map(|item| (item.uri == file_url).then_some(item.id))
        else {
            panic!("current file_url {file_url:?} not found in model items {items:?}");
        };

        let item_urls: Vec<_> = items.iter().map(|item| &item.uri).collect();
        assert_eq!(&item_urls, &file_urls[0..=loop_index]);

        model.set_current_playing(set_playing_id, PlayState::Playing);

        pusher.fill_determined()?;

        {
            let (runner, now_playing) = push_playlist_update(pusher, model)?;
            now_playing.assert_playing(expected_beet_items);
            runner.assert_none_contains(&file_url_encoded);
            // TODO remove if not wanting to test specifics
            // runner.assert_requests(&[
            //     // rustfmt hint
            //     "/requests/status.json",
            //     "/requests/playlist.json",
            // ]);
        }
    }

    Ok(())
}

fn push_playlist_update<'a>(
    pusher: &mut BeetPusher<'_, PanicRng>,
    model: &'a mut Model,
) -> eyre::Result<(ExpectHttp<'a>, NowPlaying)> {
    let mut runner = ExpectHttp::new(model);
    let mut now_playing = NowPlaying::default();

    pusher.push_playlist_update(&mut runner, Some(&mut now_playing))?;

    Ok((runner, now_playing))
}

#[test]
#[ignore = "TODO"]
fn empty_beet_fails() -> eyre::Result<()> {
    todo!()
}
