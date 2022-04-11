// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::Action;
use std::cmp::Ordering;
use std::time::Duration;

pub type Need = Option<(Option<Duration>, Action)>;
pub fn ord(lhs: &Need, rhs: &Need) -> Ordering {
    use Ordering::{Equal, Greater, Less};
    match (lhs, rhs) {
        (None, None) => Equal,
        (Some(_), None) => Less, // Some(need) is always sooner
        (None, Some(_)) => Greater,
        (Some(lhs), Some(rhs)) => match (lhs, rhs) {
            ((None, _), (None, _)) => Equal,
            ((Some(_), _), (None, _)) => Greater, // Some(duration) is always LATER! than no delay
            ((None, _), (Some(_), _)) => Less,
            ((Some(lhs), _), (Some(rhs), _)) => lhs.cmp(rhs),
        },
    }
}

#[cfg(test)]
pub mod tests {
    use super::ord as ord_need;
    use super::{Action, Duration, Need, Ordering};

    pub fn immediate(action: Action) -> Need {
        Some((None, action))
    }
    pub fn some_millis(millis: u64, action: Action) -> Need {
        Some((Some(Duration::from_millis(millis)), action))
    }
    pub fn some_millis_action(millis: u64) -> Need {
        some_millis(millis, Action::fetch_playlist_info())
    }
    pub fn immediate_action() -> Need {
        immediate(Action::fetch_playlist_info())
    }

    #[test]
    fn sorts_need_before_none() {
        let some_need = some_millis_action(1);
        assert_eq!(ord_need(&some_need, &None), Ordering::Less);
        assert_eq!(ord_need(&None, &some_need), Ordering::Greater);
    }
    #[test]
    fn sorts_need_immediate_before_delay() {
        let now = immediate_action();
        let sooner = some_millis_action(5);
        let later = some_millis_action(50);
        assert_eq!(ord_need(&now, &sooner), Ordering::Less);
        assert_eq!(ord_need(&sooner, &later), Ordering::Less);
        assert_eq!(ord_need(&now, &later), Ordering::Less);
        //
        assert_eq!(ord_need(&sooner, &now), Ordering::Greater);
        assert_eq!(ord_need(&later, &sooner), Ordering::Greater);
        assert_eq!(ord_need(&later, &now), Ordering::Greater);
    }
}
