// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    query_playback::QueryPlayback, ClientState, Error, Plan, PlanConstructor, PlaybackMode, Step,
};
use crate::{client_state::ClientStateSequence, Command};
use tracing::{debug, trace};

#[derive(Clone)]
pub(crate) struct Set {
    target: PlaybackMode,
    query_playback: QueryPlayback,
}

impl Plan for Set {
    type Output<'a> = ();

    fn next(&mut self, state: &ClientState) -> Result<Step<()>, Error> {
        let status = match self.query_playback.next(state)? {
            Step::Done(status) => status,
            Step::Need(endpoint) => return Ok(Step::Need(endpoint)),
        };

        if status.is_random != self.target.is_random() {
            debug!(
                is_random = status.is_random,
                target = self.target.is_random(),
                "want to toggle random",
            );
            return Ok(Step::Need(Command::ToggleRandom.into()));
        }

        if status.is_loop_all != self.target.is_loop_all() {
            debug!(
                is_loop_all = status.is_loop_all,
                target = self.target.is_loop_all(),
                "want to toggle loop-all",
            );
            return Ok(Step::Need(Command::ToggleLoopAll.into()));
        }

        if status.is_repeat_one != self.target.is_repeat_one() {
            debug!(
                is_loop_all = status.is_repeat_one,
                target = self.target.is_repeat_one(),
                "want to toggle repeat-one",
            );
            return Ok(Step::Need(Command::ToggleRepeatOne.into()));
        }

        trace!("no change for playback_mode");

        Ok(Step::Done(()))
    }
}
impl PlanConstructor for Set {
    type Args = PlaybackMode;
    fn new(target: Self::Args, state: ClientStateSequence) -> Self {
        Self {
            target,
            query_playback: QueryPlayback::new((), state),
        }
    }
}
impl std::fmt::Debug for Set {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Set").field(&self.target).finish()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{goal::RepeatMode, Change, Response};
    use std::str::FromStr as _;
    use test_log::test;

    fn plan<'a>(mode: PlaybackMode, state: &ClientState) -> impl Plan<Output<'a> = ()> + 'static {
        Change::PlaybackMode(mode).into_plan(state.get_ref())
    }

    trait ResultExt<T, E> {
        fn display_err(self) -> Result<T, String>;
    }
    impl<T, E> ResultExt<T, E> for Result<T, E>
    where
        E: std::fmt::Display,
    {
        fn display_err(self) -> Result<T, String> {
            self.map_err(|e| format!("{e}"))
        }
    }

    fn status(mode: PlaybackMode) -> Response {
        let status = serde_json::json!({
          "rate":1,
          "time":0,
          "repeat": mode.is_repeat_one(),
          "loop": mode.is_loop_all(),
          "length":0,
          "random": mode.is_random(),
          "apiversion":3,
          "version":"3.0.20 Vetinari",
          "currentplid":438,
          "position":0.0,
          "volume":256,
          "state":"playing",
          "information":{"category":{"meta":{}}},
        });
        Response::from_str(&status.to_string()).expect("valid response JSON")
    }

    #[test]
    fn sets_random() {
        let mut state = ClientState::new();

        let default = PlaybackMode::default();
        let random = default.set_random(true);

        let mut action_default = plan(default, &state);
        let mut action_random = plan(random, &state);

        // all require the status
        assert_eq!(
            action_default.next(&state).unwrap(),
            action_random.next(&state).unwrap()
        );
        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        state.update(status(default));

        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @"Done(())");
        insta::assert_ron_snapshot!(action_random.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_random",
        ))
        "###);

        state.update(status(default.set_random(true)));

        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_random",
        ))
        "###);
        insta::assert_ron_snapshot!(action_random.next(&state).unwrap(), @"Done(())");
    }

    #[test]
    fn sets_repeat_one() {
        let mut state = ClientState::new();

        let default = PlaybackMode::default();
        let one = default.set_repeat(RepeatMode::One);
        let all = default.set_repeat(RepeatMode::All);

        let mut action_default = plan(default, &state);
        let mut action_one = plan(one, &state);
        let mut action_all = plan(all, &state);

        // all require the status
        assert_eq!(
            action_default.next(&state).unwrap(),
            action_one.next(&state).unwrap()
        );
        assert_eq!(
            action_default.next(&state).unwrap(),
            action_all.next(&state).unwrap()
        );
        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        state.update(status(default));

        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @"Done(())");
        insta::assert_ron_snapshot!(action_one.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_repeat",
        ))
        "###);
        insta::assert_ron_snapshot!(action_all.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);

        state.update(status(default.set_repeat(RepeatMode::One)));

        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_repeat",
        ))
        "###);
        insta::assert_ron_snapshot!(action_one.next(&state).unwrap(), @"Done(())");
        insta::assert_ron_snapshot!(action_all.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);

        state.update(status(default.set_repeat(RepeatMode::All)));

        insta::assert_ron_snapshot!(action_default.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);
        insta::assert_ron_snapshot!(action_one.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);
        insta::assert_ron_snapshot!(action_all.next(&state).unwrap(), @"Done(())");
    }

    #[test]
    fn panics_mismatched_instances() {
        let state1 = ClientState::new();
        let state2 = ClientState::new();

        let default = PlaybackMode::default();

        let mut action_on_1 = plan(default, &state1);
        let mut action_on_2 = plan(default, &state2);

        insta::assert_ron_snapshot!(action_on_1.next(&state1).display_err(), @r###"
        Ok(Need(Endpoint(
          path_and_query: "/requests/status.json",
        )))
        "###);
        insta::assert_ron_snapshot!(action_on_2.next(&state2).display_err(), @r###"
        Ok(Need(Endpoint(
          path_and_query: "/requests/status.json",
        )))
        "###);

        insta::assert_ron_snapshot!(action_on_1.next(&state2).display_err(), @r###"Err("action shared among multiple client instances")"###);
        insta::assert_ron_snapshot!(action_on_2.next(&state1).display_err(), @r###"Err("action shared among multiple client instances")"###);
    }
}
