// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::index_item::DesiredType;
use super::{index_item, Command, Converter};
use crate::controller::LowCommand;
use crate::vlc_responses::PlaylistItem;
use std::num::NonZeroUsize;

impl Converter {
    /// Emits a `PlaylistDelete` error for the first item in `items`
    /// when `max_item_count` is exceeded by:
    /// * The item count before the current, desired item
    /// * The item count including the previous, desired item
    /// * The item count including the current (non-desired) item
    /// * For no current item, the total item count
    pub(super) fn check_remove_first(
        items: &[PlaylistItem],
        current_desired_index: Option<(index_item::Type, usize)>,
        command: &Command,
    ) -> Result<(), LowCommand> {
        let max_history_count = {
            let max_history_count: NonZeroUsize = command.max_history_count;
            usize::from(max_history_count)
        };
        let items_count = current_desired_index.map_or_else(
            || items.len(),
            |(desired_type, index)| {
                match desired_type {
                    Some(DesiredType::Current) => index,             //count BEFORE
                    Some(DesiredType::Previous) | None => index + 1, //count INCLUDING
                }
            },
        );
        (items_count <= max_history_count)
            .then_some(())
            .ok_or_else(|| {
                let first = items
                    .first()
                    .expect("first exists for length exceeding nonzero bound");
                LowCommand::PlaylistDelete {
                    item_id: first.id.to_string(),
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Command, Converter};
    use crate::{
        command::LowCommand, controller::high_converter::playlist_set::index_item::DesiredType,
        vlc_responses::PlaylistItem,
    };

    macro_rules! assert_remove {
        (
            items = $items:expr;
            current = $current:expr;
            max_history_count = $max_history_count:expr;
            $expected:expr
        ) => {{
            let items: &[PlaylistItem] = $items;
            let items_debug: Vec<_> = items
                .iter()
                .map(|item| format!("{}=>{}", item.id, item.url.to_string()))
                .collect();
            let current: Option<(
                crate::controller::high_converter::playlist_set::index_item::Type,
                usize,
            )> = $current;
            let max_history_count: usize = $max_history_count;
            let command = command_with_max_history_count(max_history_count);
            assert_eq!(
                Converter::check_remove_first(items, current, &command),
                $expected.into(),
                "items {items_debug:?}, current {current:?}, max_history_count {max_history_count}"
            );
        }};
    }
    fn command_with_max_history_count(max_history_count: usize) -> Command {
        Command {
            urls: vec![],
            max_history_count: max_history_count.try_into().expect("nonzero"),
        }
    }

    #[test]
    fn ignores_empty() {
        assert_remove!(
            items = &items![];
            current = None;
            max_history_count = 1;
            Ok(())
        );
    }
    #[test]
    fn one() {
        for item_num in 0..100 {
            assert_remove!(
                items = &items![item_num => "alpha", 200 => "beta"];
                current = None;
                max_history_count = 1;
                Err(delete(item_num))
            );
        }
    }
    #[test]
    fn delete_no_current() {
        let items_full = &items!["alpha", "beta", "cauli", "deno", "edifice", "fence", "gentry"];
        for len in 2..items_full.len() {
            let items = &items_full[..len];
            assert_remove!(
                items = items;
                current = None;
                max_history_count = items.len() - 1;
                Err(delete(0))
            );
        }
    }
    #[test]
    fn keeps_before_current() {
        for max_history_count in 1..20 {
            for first_id in 0..10 {
                let expected = (max_history_count >= 3)
                    .then_some(())
                    .ok_or_else(|| delete(first_id));
                assert_remove!(
                    items = &items![first_id=>"go", 10=>"dog", 20=>"spot", 31=>"last"];
                    current = None;
                    max_history_count = max_history_count + 1;
                    expected.clone()
                );
                assert_remove!(
                    items = &items![first_id=>"go", 10=>"dog", 20=>"spot", 31=>"last"];
                    current = Some((Some(DesiredType::Current), 3));
                    max_history_count = max_history_count;
                    expected
                );
            }
        }
    }
    #[test]
    fn keeps_no_current() {
        for max_history_count in 1..20 {
            for first_id in 0..10 {
                let expected = (max_history_count >= 4)
                    .then_some(())
                    .ok_or_else(|| delete(first_id));
                assert_remove!(
                    items = &items![first_id=>"go", 10=>"dog", 20=>"spot", 31=>"runs"];
                    current = None;
                    max_history_count = max_history_count;
                    expected
                );
            }
        }
    }

    fn delete(id: usize) -> LowCommand {
        LowCommand::PlaylistDelete {
            item_id: id.to_string(),
        }
    }
    #[test]
    fn counts_before_desired_current() {
        let first = 6;
        let items = &items![first=>"a", 1=>"b", 2=>"c", 3=>"d"];
        let current = |index| Some((Some(DesiredType::Current), index));
        for ok in [Err(()), Ok(())] {
            let ok_plus_one = if ok.is_ok() { 1 } else { 0 };
            assert_remove!(
                items = items;
                current = current(2);
                max_history_count = 1 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
            assert_remove!(
                items = items;
                current = current(3);
                max_history_count = 2 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
        }
    }
    #[test]
    fn counts_before_undesired_current() {
        let first = 6;
        let items = &items![first=>"a", 1=>"b", 2=>"c", 3=>"d"];
        let undesired = |index| Some((None, index));
        for ok in [Err(()), Ok(())] {
            let ok_plus_one = if ok.is_ok() { 1 } else { 0 };
            assert_remove!(
                items = items;
                current = undesired(1);
                max_history_count = 1 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
            assert_remove!(
                items = items;
                current = undesired(2);
                max_history_count = 2 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
            assert_remove!(
                items = items;
                current = undesired(3);
                max_history_count = 3 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
        }
    }
    #[test]
    fn counts_including_desired_previous() {
        let first = 6;
        let items = &items![first=>"a", 1=>"b", 2=>"c", 3=>"d"];
        let previous = |index| Some((Some(DesiredType::Previous), index));
        for ok in [Err(()), Ok(())] {
            let ok_plus_one = if ok.is_ok() { 1 } else { 0 };
            assert_remove!(
                items = items;
                current = previous(1);
                max_history_count = 1 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
            assert_remove!(
                items = items;
                current = previous(2);
                max_history_count = 2 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
            assert_remove!(
                items = items;
                current = previous(3);
                max_history_count = 3 + ok_plus_one;
                ok.map_err(|()| delete(first))
            );
        }
    }
}
