// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    query_playback::QueryPlayback, ClientState, Error, PlaybackMode, Poll, Pollable,
    PollableConstructor,
};
use crate::Command;

pub(crate) struct Set {
    target: PlaybackMode,
    query_playback: QueryPlayback,
}

impl Pollable for Set {
    type Output<'a> = ();

    fn next(&mut self, state: &ClientState) -> Result<Poll<()>, Error> {
        let status = match self.query_playback.next(state)? {
            Poll::Done(status) => status,
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        };

        if status.is_random != self.target.is_random() {
            return Ok(Poll::Need(Command::ToggleRandom.into()));
        }

        if status.is_loop_all != self.target.is_loop_all() {
            return Ok(Poll::Need(Command::ToggleLoopAll.into()));
        }

        if status.is_repeat_one != self.target.is_repeat_one() {
            return Ok(Poll::Need(Command::ToggleRepeatOne.into()));
        }

        Ok(Poll::Done(()))
    }
}
impl PollableConstructor for Set {
    type Args = PlaybackMode;
    fn new(target: Self::Args, state: &ClientState) -> Self {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{action::RepeatMode, Action, Response};
    use std::str::FromStr as _;

    fn action<'a>(
        mode: PlaybackMode,
        state: &ClientState,
    ) -> impl Pollable<Output<'a> = ()> + 'static {
        Action::PlaybackMode(mode).pollable(state)
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

        let mut action_default = action(default, &state);
        let mut action_random = action(random, &state);

        // all require the status
        assert_eq!(action_default.next(&state), action_random.next(&state));
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

        let mut action_default = action(default, &state);
        let mut action_one = action(one, &state);
        let mut action_all = action(all, &state);

        // all require the status
        assert_eq!(action_default.next(&state), action_one.next(&state));
        assert_eq!(action_default.next(&state), action_all.next(&state));
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

        let mut action_on_1 = action(default, &state1);
        let mut action_on_2 = action(default, &state2);

        insta::assert_ron_snapshot!(action_on_1.next(&state1), @r###"
        Ok(Need(Endpoint(
          path_and_query: "/requests/status.json",
        )))
        "###);
        insta::assert_ron_snapshot!(action_on_2.next(&state2), @r###"
        Ok(Need(Endpoint(
          path_and_query: "/requests/status.json",
        )))
        "###);

        insta::assert_ron_snapshot!(action_on_1.next(&state2), @"Err(InvalidClientInstance)");
        insta::assert_ron_snapshot!(action_on_2.next(&state1), @"Err(InvalidClientInstance)");
    }
}
