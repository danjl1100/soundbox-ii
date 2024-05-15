// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{ClientState, Endpoint, PlaybackMode, Pollable, PollableConstructor};
use crate::Command;

#[derive(Debug)]
pub(crate) struct Set(PlaybackMode);

impl Pollable for Set {
    type Output<'a> = ();

    fn next_endpoint<'a>(&mut self, state: &'a ClientState) -> Result<Endpoint, Self::Output<'a>> {
        let playback = state.playback_status();

        let Some(status) = playback.as_ref() else {
            return Ok(Endpoint::query_status());
        };

        if status.is_random != self.0.is_random() {
            return Ok(Command::ToggleRandom.into());
        }

        if status.is_loop_all != self.0.is_loop_all() {
            return Ok(Command::ToggleLoopAll.into());
        }

        if status.is_repeat_one != self.0.is_repeat_one() {
            return Ok(Command::ToggleRepeatOne.into());
        }

        Err(())
    }
}
impl PollableConstructor for Set {
    type Args = PlaybackMode;
    fn new(target: Self::Args, _state: &ClientState) -> Self {
        Self(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{action::RepeatMode, Action, Response};
    use std::str::FromStr as _;

    fn action<'a>(mode: PlaybackMode) -> impl Pollable<Output<'a> = ()> + 'static {
        Action::PlaybackMode(mode).pollable(&ClientState::new())
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

        let mut action_default = action(default);
        let mut action_random = action(random);

        // all require the status
        assert_eq!(
            action_default.next_endpoint(&state),
            action_random.next_endpoint(&state),
        );
        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        state.update(status(default));

        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @"Err(())");
        insta::assert_ron_snapshot!(action_random.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_random",
        ))
        "###);

        state.update(status(default.set_random(true)));

        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_random",
        ))
        "###);
        insta::assert_ron_snapshot!(action_random.next_endpoint(&state), @"Err(())");
    }

    #[test]
    fn sets_repeat_one() {
        let mut state = ClientState::new();

        let default = PlaybackMode::default();
        let one = default.set_repeat(RepeatMode::One);
        let all = default.set_repeat(RepeatMode::All);

        let mut action_default = action(default);
        let mut action_one = action(one);
        let mut action_all = action(all);

        // all require the status
        assert_eq!(
            action_default.next_endpoint(&state),
            action_one.next_endpoint(&state),
        );
        assert_eq!(
            action_default.next_endpoint(&state),
            action_all.next_endpoint(&state),
        );
        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        state.update(status(default));

        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"Err(())"###);
        insta::assert_ron_snapshot!(action_one.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_repeat",
        ))
        "###);
        insta::assert_ron_snapshot!(action_all.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);

        state.update(status(default.set_repeat(RepeatMode::One)));

        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_repeat",
        ))
        "###);
        insta::assert_ron_snapshot!(action_one.next_endpoint(&state), @"Err(())");
        insta::assert_ron_snapshot!(action_all.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);

        state.update(status(default.set_repeat(RepeatMode::All)));

        insta::assert_ron_snapshot!(action_default.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);
        insta::assert_ron_snapshot!(action_one.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/status.json?command=pl_loop",
        ))
        "###);
        insta::assert_ron_snapshot!(action_all.next_endpoint(&state), @"Err(())");
    }
}
