//! Tests the logic from fake injected beet responses up to the VLC http requests

use self::expect_beet::ExpectBeet;
use self::expect_http::ExpectHttp;
use beet_pusher::{BeetItem, BeetPusher, NowPlayingObserver};
use std::sync::{Arc, Mutex};

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
    use vlc_http::sync::EndpointRequestor;

    pub struct ExpectHttp {}
    impl ExpectHttp {
        pub fn new() -> Self {
            Self {}
        }
        pub fn assert_empty(self) {
            todo!()
        }
    }
    impl EndpointRequestor for ExpectHttp {
        type Error = std::convert::Infallible;

        fn request(
            &mut self,
            endpoint: vlc_http::Endpoint,
        ) -> Result<vlc_http::Response, Self::Error> {
            dbg!(endpoint);
            todo!()
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NowPlaying {
    items: Arc<Mutex<Vec<BeetItem>>>,
}
impl NowPlaying {
    #[track_caller]
    fn assert_and_clear(&self, expected: &str) {
        let expected = expected
            .lines()
            .map(str::parse)
            .collect::<Result<Vec<BeetItem>, _>>()
            .expect("valid expected beet items string");

        let mut items = self.items.lock().expect("mutex lock");
        assert_eq!(*items, expected);
        items.clear();
    }
}
impl NowPlayingObserver for NowPlaying {
    type Error = std::convert::Infallible;

    fn now_playing(&mut self, item: &BeetItem) -> Result<(), Self::Error> {
        self.items.lock().expect("mutex lock").push(item.clone());
        Ok(())
    }
}

fn new_test_beet_pusher(
    spigot: bucket_spigot::Network<BeetItem, String>,
) -> (BeetPusher<'static, PanicRng, NowPlaying>, NowPlaying) {
    let base_url = beet_pusher::BaseUrl("file://host/".parse().expect("valid URL"));
    let rng = Box::leak(Box::new(PanicRng));

    let now_playing = NowPlaying::default();
    let pusher =
        BeetPusher::new(rng, spigot, base_url).set_now_playing_observer(now_playing.clone());

    (pusher, now_playing)
}

#[test]
#[ignore = "TODO"]
fn queries_beet_for_buckets() -> eyre::Result<()> {
    let spigot = bucket_spigot::Network::<BeetItem, String>::from_commands_str_whitespace(
        "
    add-bucket .
    ",
    )
    .expect("valid script");

    let (mut pusher, now_playing) = new_test_beet_pusher(spigot);

    {
        let runner = ExpectBeet::new(&[(
            &["ls", "-f", "="],
            "1=first
2=second",
        )]);
        pusher.fill_buckets(&runner)?;
        runner.assert_empty();
    }

    // TODO: when is this necessary???
    // pusher.fill_determined()?;

    {
        let mut runner = ExpectHttp::new();
        pusher.push_playlist_update(&mut runner)?;
        runner.assert_empty();
    }

    now_playing.assert_and_clear(
        "1=first
2=secondd
3=S.I.C.",
    );

    todo!()
}

#[test]
#[ignore = "TODO"]
fn empty_beet_fails() -> eyre::Result<()> {
    todo!()
}
