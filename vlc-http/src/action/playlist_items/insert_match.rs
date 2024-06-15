// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

/// Search for the *beginning* of `target` at the *end* of `existing`, possibly with interspersed
/// extra undesired elements in `existing`.
///
/// Returns the match index and the next element in `target` to append to `existing`,
/// for the goal of `existing` to end with all elements of `target` in-order
pub(super) fn find_insert_match<'a, T>(target: &'a [T], existing: &[&T]) -> InsertMatch<'a, T>
where
    T: Eq + std::fmt::Debug,
{
    for match_start in 0..existing.len() {
        let existing = &existing[match_start..];

        let mut target_iter = target.iter();
        let mut existing_iter = existing.iter().copied().enumerate();

        let target_first = target_iter.next();
        let existing_first = existing_iter.next().map(|(_, elem)| elem);

        // TODO remove debug
        println!("match_start = {match_start}, target_first = {target_first:?}, existing_first = {existing_first:?}");

        if target_first == existing_first {
            let next = loop {
                let Some(target_elem) = target_iter.next() else {
                    if let Some((existing_index, _)) = existing_iter.next() {
                        // extra item, delete
                        break Some(MatchAction::DeleteIndex(match_start + existing_index));
                    }
                    // ended at the same time, no action
                    break None;
                };
                let existing_elem = existing_iter.next();
                // TODO remove debug
                println!(
                    "target_elem = {target_elem:?}, existing_elem = {:?}",
                    existing_elem.map(|(_, elem)| elem)
                );
                match existing_elem {
                    // equal, continue search
                    Some((_, existing_elem)) if existing_elem == target_elem => continue,
                    // non-equal, delete the offending item
                    Some((existing_index, _)) => {
                        break Some(MatchAction::DeleteIndex(match_start + existing_index));
                    }
                    // missing, add new
                    None => {
                        break Some(MatchAction::InsertValue(target_elem));
                    }
                }
            };
            return InsertMatch {
                match_start: Some(match_start),
                next,
            };
        }
    }

    // no partial matches found, begin by adding the first (if any)
    InsertMatch {
        match_start: None,
        next: target.first().map(MatchAction::InsertValue),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InsertMatch<'a, T> {
    pub match_start: Option<usize>,
    pub next: Option<MatchAction<'a, T>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MatchAction<'a, T> {
    InsertValue(&'a T),
    DeleteIndex(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_end<T>(next: &T) -> InsertMatch<'_, T> {
        InsertMatch {
            match_start: None,
            next: Some(MatchAction::InsertValue(next)),
        }
    }
    fn insert_from<T>(match_start: usize, next: &T) -> InsertMatch<'_, T> {
        InsertMatch {
            match_start: Some(match_start),
            next: Some(MatchAction::InsertValue(next)),
        }
    }
    fn matched<T>(match_start: usize) -> InsertMatch<'static, T> {
        InsertMatch {
            match_start: Some(match_start),
            next: None,
        }
    }
    fn matched_delete<T>(match_start: usize, delete: usize) -> InsertMatch<'static, T> {
        InsertMatch {
            match_start: Some(match_start),
            next: Some(MatchAction::DeleteIndex(delete)),
        }
    }

    // NOTE tests are easier to read with this alias
    fn uut<'a, T>(target: &'a [T], existing: &[&T]) -> InsertMatch<'a, T>
    where
        T: std::fmt::Debug + Eq,
    {
        println!("target={target:?}, existing={existing:?}");
        find_insert_match(target, existing)
    }

    const X: &i32 = &10;

    #[test]
    fn find_next() {
        let needle = &[1, 2, 3, 4];
        assert_eq!(uut(needle, &[]), insert_end(&1));
        assert_eq!(uut(needle, &[&1]), insert_from(0, &2));
        assert_eq!(uut(needle, &[&1, &2]), insert_from(0, &3));
        assert_eq!(uut(needle, &[&1, &2, &3]), insert_from(0, &4));
        assert_eq!(uut(needle, &[&1, &2, &3, &4]), matched(0));
        assert_eq!(uut(needle, &[X, &1, &2, &3, &4]), matched(1));
        assert_eq!(uut(needle, &[X, X, &1, &2, &3, &4]), matched(2));
        //                        0   1   2   3  [4]
        assert_eq!(uut(needle, &[&1, &2, &3, &4, &1]), matched_delete(0, 4));
        //                       0  1 [2]
        assert_eq!(uut(needle, &[X, X, &1, &2]), insert_from(2, &3));
    }

    #[test]
    fn find_interspersed() {
        let needle = &[1, 2, 3];
        assert_eq!(uut(needle, &[X, &1, X, &2]), matched_delete(1, 2));
        assert_eq!(uut(needle, &[X, &1, X, &2, X]), matched_delete(1, 2));
        assert_eq!(uut(needle, &[X, &1, &2, X]), matched_delete(1, 3));
        assert_eq!(uut(needle, &[X, &1, &2, X, &3]), matched_delete(1, 3));
        assert_eq!(uut(needle, &[X, &1, &2, &3, X]), matched_delete(1, 4));
        assert_eq!(uut(needle, &[X, &1, &2, &3]), matched(1));
    }
}
