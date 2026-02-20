// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tests the logic from fake injected beet responses up to the VLC http requests

use self::expect_beet::ExpectBeet;
use self::expect_http::ExpectHttp;
use beet_pusher::{BeetItem, BeetPusher, NowPlayingObserver};
use std::str::FromStr;
use vlc_http::testing::{Model, PlayState};

struct PanicRng;
impl rand::RngCore for PanicRng {
    fn next_u32(&mut self) -> u32 {
        unimplemented!("PanicRng")
    }

    fn next_u64(&mut self) -> u64 {
        unimplemented!("PanicRng")
    }

    fn fill_bytes(&mut self, _: &mut [u8]) {
        unimplemented!("PanicRng")
    }

    fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand::Error> {
        unimplemented!("PanicRng")
    }
}

mod expect_beet {
    use beet_pusher::BeetRunner;
    use std::{cell::Cell, collections::VecDeque, process::ExitStatus};

    pub struct ExpectBeet {
        inner: Cell<ExpectBeetInner>,
    }
    #[derive(Debug, Default)]
    pub struct ExpectBeetInner {
        args_to_results: VecDeque<(&'static [&'static str], &'static str)>,
        // pub args_per_call: Vec<Vec<String>>,
    }
    impl ExpectBeet {
        pub fn new(args_to_results: &[(&'static [&'static str], &'static str)]) -> Self {
            let args_to_results = args_to_results.to_vec().into();
            Self {
                inner: Cell::new(ExpectBeetInner { args_to_results }),
            }
        }
        pub fn assert_empty(self) {
            let Self { inner } = self;
            let inner = inner.into_inner();

            let ExpectBeetInner { args_to_results } = &inner;
            assert_eq!(args_to_results, &[]);
        }
    }
    impl BeetRunner for ExpectBeet {
        type Error = std::convert::Infallible;

        fn run_beet_command<S>(
            &self,
            args: impl Iterator<Item = S>,
        ) -> Result<std::process::Output, Self::Error>
        where
            S: AsRef<str>,
        {
            let mut inner = self.inner.take();

            let found_args: Vec<String> = args.map(|s| s.as_ref().to_string()).collect();
            let Some((expected_args, stdout)) = inner.args_to_results.pop_front() else {
                panic!("no request expected, but found: {found_args:?}")
            };
            assert_eq!(found_args, expected_args);
            // inner.args_per_call.push(args);

            self.inner.replace(inner);

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
        #[track_caller]
        pub fn assert_requests(self, expected_requests: &[&str]) {
            let Self { model: _, requests } = self;
            assert_eq!(requests, expected_requests);
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

#[test]
fn queries_beet_for_buckets() -> eyre::Result<()> {
    let spigot = bucket_spigot::Network::<BeetItem, String>::from_commands_str_whitespace(
        "
    add-bucket .
    ",
    )
    .expect("valid script");

    let beet_items_str = "1=first
2=second
3=third";

    let file_urls = [
        //
        "file://host/first",
        "file://host/second",
        "file://host/third",
    ];

    let pusher = &mut new_test_beet_pusher(spigot);
    let model = &mut Model::default();

    {
        let runner = ExpectBeet::new(&[(&["ls", "-f", "="], beet_items_str)]);
        pusher.fill_buckets(&runner)?;
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
            runner.assert_requests(&[
                "/requests/status.json",
                "/requests/playlist.json",
                &format!("/requests/playlist.json?command=in_enqueue&input={file_url_encoded}"),
            ]);
            now_playing.assert_playing(&[]);
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

        {
            let (runner, now_playing) = push_playlist_update(pusher, model)?;
            runner.assert_requests(&[
                // rustfmt hint
                "/requests/status.json",
                "/requests/playlist.json",
            ]);
            now_playing.assert_playing(expected_beet_items);
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
