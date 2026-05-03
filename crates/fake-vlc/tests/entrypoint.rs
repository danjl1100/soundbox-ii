// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tests the `FakeVlc` server itself

use fake_vlc::FakeVlc;

#[test]
fn arb_playlist_set_and_get() -> eyre::Result<()> {
    let (vlc, thread_handle) = FakeVlc::new()?;

    // arbtest::arbtest(|u| {
    //     // META TODO: vlc-http-cmds structure is too opaque to figure out the set of available high-level commands
    //     // iterate inside `cargo doc` to make it clearer from IDE's autocomplete

    //     // TODO: arbitrary playlist, then set it using ureq
    //     // let target: vlc_http_cmd::goal::TargetPlaylistItems = u.arbitrary()?;

    //     // TODO verify FakeVlc reports a playlist matching the target
    //     // TODO verify the query-result playlist matches the target

    //     Ok(())
    // });

    drop(vlc);
    thread_handle.join().expect("VLC thread panic")?;
    // TODO
    Ok(())
}

#[test]
#[ignore = "TODO"]
fn arb_action_succeeds() -> eyre::Result<()> {
    todo!()
}
