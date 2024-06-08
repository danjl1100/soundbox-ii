// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::*;

#[test]
fn playlist_add() {
    let normal_url = "file://this/is/a/url.mp4".parse().expect("url");
    let small_url = "file://.".parse().expect("url");
    let weird_url = url::Url::parse("file:///SENTINEL_%20_URL_%20%5E%24").expect("valid url");

    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistAdd {
        url: normal_url
    }), @r###"
    Endpoint(
      path_and_query: "/requests/playlist.json?command=in_enqueue&input=file://this/is/a/url.mp4",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistAdd {
        url: small_url,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/playlist.json?command=in_enqueue&input=file://./",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistAdd {
        url: weird_url,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/playlist.json?command=in_enqueue&input=file:///SENTINEL_%20_URL_%20%5E%24",
    )
    "###);
}

#[test]
fn playlist_delete() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistDelete {
        item_id: 123,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/playlist.json?command=pl_delete&id=123",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistDelete {
        item_id: 0
    }), @r###"
    Endpoint(
      path_and_query: "/requests/playlist.json?command=pl_delete&id=0",
    )
    "###);
}

#[test]
fn playlist_play() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistPlay {
        item_id: None,
    }),
    @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_play",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistPlay {
        item_id: Some(456),
    }),
    @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_play&id=456",
    )
    "###);
}

#[test]
fn toggle_random() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::ToggleRandom), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_random",
    )
    "###);
}
#[test]
fn toggle_repeat_one() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::ToggleRepeatOne), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_repeat",
    )
    "###);
}
#[test]
fn toggle_loop_all() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::ToggleLoopAll), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_loop",
    )
    "###);
}

#[test]
fn playback_resume() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackResume), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_forceresume",
    )
    "###);
}
#[test]
fn playback_pause() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackPause), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_forcepause",
    )
    "###);
}
#[test]
fn playback_stop() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackStop), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_stop",
    )
    "###);
}
#[test]
fn seek_next() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekNext), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_next",
    )
    "###);
}
#[test]
fn seek_previous() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekPrevious), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=pl_previous",
    )
    "###);
}

#[test]
fn seek_to() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekTo {
        seconds: 0,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=0",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekTo {
        seconds: 10,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=10",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekTo {
        seconds: 259,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=259",
    )
    "###);
}

#[test]
fn seek_relative() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekRelative {
        seconds_delta: 0.into(),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=%2B0",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekRelative {
        seconds_delta: 32.into(),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=%2B32",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::SeekRelative {
        seconds_delta: (-57).into(),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=seek&val=-57",
    )
    "###);
}

#[test]
fn volume() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 0u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=0",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 20u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=51",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 40u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=102",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 60u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=154",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 80u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=205",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 100u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=256",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 200u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=512",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::Volume {
        percent: 300u16.try_into().expect("volume"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=768",
    )
    "###);
}

#[test]
fn volume_delta_positive() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 0i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B0",
    )
    "###);

    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 20i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B51",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 40i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B102",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 60i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B154",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 80i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B205",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 100i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B256",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 200i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B512",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: 300i16.try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=%2B768",
    )
    "###);
}
#[test]
fn volume_delta_negative() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-20i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-51",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-40i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-102",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-60i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-154",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-80i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-205",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-100i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-256",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-200i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-512",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::VolumeRelative {
        percent_delta: (-300i16).try_into().expect("delta"),
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=volume&val=-768",
    )
    "###);
}

#[test]
fn playback_speed() {
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackSpeed {
        speed: 0.0,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=rate&val=0",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackSpeed {
        speed: 0.21,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=rate&val=0.21",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackSpeed {
        speed: 2.1,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=rate&val=2.1",
    )
    "###);
    insta::assert_ron_snapshot!(Endpoint::from(Command::PlaybackSpeed {
        speed: 5.91,
    }), @r###"
    Endpoint(
      path_and_query: "/requests/status.json?command=rate&val=5.91",
    )
    "###);
}
