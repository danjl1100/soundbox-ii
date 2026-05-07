// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tests the `FakeVlc` server itself

#![expect(clippy::panic, reason = "arbtest requires panic for inner errors")]

use vlc_http::ClientState;
use vlc_http_test::arb_goal::ArbGoal;

#[test]
fn arb_playlist_set_and_get() -> eyre::Result<()> {
    let mut waiting_proof_of_work = Some(());

    fake_vlc::arbtest_with_fake_vlc(|u, vlc, endpoint_caller| {
        // command arbitrary playlist, using ureq
        let target: vlc_http_test::arb_goal::ArbTargetPlaylistItems = u.arbitrary()?;

        // calculate complexity
        let max_iter_count = {
            let current_len = vlc.get_playlist_cloned().len();
            ArbGoal::from(target.clone()).get_complexity(current_len)
        };

        let target_playlist_items = vlc_http::goal::TargetPlaylistItems::from(target.clone());

        // clone items, for comparison
        let items = target_playlist_items.get_urls().to_vec();

        // execute playlist set-and-query
        let mut client_state = ClientState::new();
        let plan = client_state
            .build_plan()
            .set_playlist_and_query_matched(target_playlist_items);
        let result =
            vlc_http::sync::complete_plan(plan, &mut client_state, endpoint_caller, max_iter_count);
        let read_items = match result {
            Ok(read_items) => read_items,
            Err(err) => {
                eprintln!("{:?}", eyre::eyre!(err));
                panic!("error completing plan {target:?}");
            }
        };

        // verify `FakeVlc::get_playlist` method matches the target
        let vlc_playlist_items: Vec<_> = vlc
            .get_playlist_cloned()
            .iter()
            .map(|item| vlc_http::url::Url::parse(&item.uri).expect("valid URI"))
            .collect();
        assert_eq!(vlc_playlist_items, items);

        // verify FakeVlc HTTP query-result matches the target
        assert_eq!(
            read_items
                .iter()
                .map(|item| item.get_url().clone())
                .collect::<Vec<_>>(),
            items
        );

        dbg!(&items);
        if !items.is_empty() {
            waiting_proof_of_work.take();
        }

        Ok(())
    })
    .budget_ms(1_000)
    .run()?;

    assert!(waiting_proof_of_work.is_none());

    Ok(())
}

#[test]
#[ignore = "TODO"]
fn arb_action_succeeds() -> eyre::Result<()> {
    todo!()
}

#[test]
#[ignore = "TODO"]
fn rejects_wrong_password() -> eyre::Result<()> {
    todo!()
}
