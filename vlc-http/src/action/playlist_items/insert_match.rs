// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

/// Search for the *beginning* of `target` at the *end* of `existing`, possibly with interspersed
/// extra undesired elements in `existing`.
///
/// Returns the match index and the next element in `target` to append to `existing`,
/// for the goal of `existing` to end with all elements of `target` in-order
pub(super) fn find_insert_match<'a, 'b, T, U>(
    target: &'a [T],
    existing: &'b [U],
) -> InsertMatch<'a, T, U>
where
    T: Eq + std::fmt::Debug,
    U: AsRef<T>,
    'b: 'a,
{
    #[cfg(test)]
    println!(
        "find_insert_match, target={target:?}, existing={existing:?}",
        existing = existing.iter().map(AsRef::as_ref).collect::<Vec<_>>()
    );
    for match_start in 0..existing.len() {
        let existing = &existing[match_start..];

        let mut target_iter = target.iter();
        let mut existing_iter = existing.iter();

        let target_first = target_iter.next();
        let existing_first = existing_iter.next().map(AsRef::as_ref);

        {
            #[cfg(test)]
            println!("match_start = {match_start}, target_first = {target_first:?}, existing_first = {existing_first:?}");
        }

        if target_first == existing_first {
            let next = loop {
                let Some(target_elem) = target_iter.next() else {
                    if let Some(existing_elem) = existing_iter.next() {
                        // extra item, delete
                        break Some(MatchAction::DeleteValue(existing_elem));
                    }
                    // ended at the same time, no action
                    break None;
                };
                let existing_elem = existing_iter.next();

                {
                    #[cfg(test)]
                    println!(
                        "target_elem = {target_elem:?}, existing_elem = {:?}",
                        existing_elem.map(AsRef::as_ref),
                    );
                }

                match existing_elem {
                    // equal, continue search
                    Some(existing_elem) if target_elem == existing_elem.as_ref() => continue,
                    // non-equal, delete the offending item
                    Some(existing_elem) => {
                        {
                            #[cfg(test)]
                            println!("wanting to delete {:?}", existing_elem.as_ref());
                        }

                        break Some(MatchAction::DeleteValue(existing_elem));
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
pub(super) struct InsertMatch<'a, T, U> {
    pub match_start: Option<usize>,
    pub next: Option<MatchAction<'a, T, U>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MatchAction<'a, T, U> {
    InsertValue(&'a T),
    DeleteValue(&'a U),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_end<T>(next: &T) -> InsertMatch<'_, T, Indexed<T>> {
        InsertMatch {
            match_start: None,
            next: Some(MatchAction::InsertValue(next)),
        }
    }
    fn insert_from<T>(match_start: usize, next: &T) -> InsertMatch<'_, T, Indexed<T>> {
        InsertMatch {
            match_start: Some(match_start),
            next: Some(MatchAction::InsertValue(next)),
        }
    }
    fn matched<T>(match_start: usize) -> InsertMatch<'static, T, Indexed<T>> {
        InsertMatch {
            match_start: Some(match_start),
            next: None,
        }
    }
    fn matched_delete<T>(
        match_start: usize,
        delete: &Indexed<T>,
    ) -> InsertMatch<'_, T, Indexed<T>> {
        InsertMatch {
            match_start: Some(match_start),
            next: Some(MatchAction::DeleteValue(delete)),
        }
    }

    // NOTE tests are easier to read with this alias
    fn uut<'a, 'b, T, U>(target: &'a [T], existing: &'b [U]) -> InsertMatch<'a, T, U>
    where
        T: std::fmt::Debug + Eq,
        U: AsRef<T>,
        'b: 'a,
    {
        println!(
            "target={target:?}, existing={existing:?}",
            existing = existing.iter().map(AsRef::as_ref).collect::<Vec<_>>()
        );
        find_insert_match(target, existing)
    }

    macro_rules! assert_eq_uut {
        ($needle:expr, &[$($elem:expr),* $(,)?]; $expected:expr) => {{
            let needle: &[i32] = $needle;
            let existing: Vec<Indexed<i32>> = [$($elem),*].into_iter()
                .enumerate()
                .map(|(index, value)| Indexed { value, index })
                .collect::<Vec<_>>();
            let existing: &[Indexed<i32>] = &existing;
            assert_eq!(uut(needle, existing), $expected);
        }};
    }
    #[derive(PartialEq, Eq)]
    struct Indexed<T> {
        value: T,
        index: usize,
    }
    impl<T> std::fmt::Debug for Indexed<T>
    where
        T: std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            <T as std::fmt::Debug>::fmt(&self.value, f)
        }
    }
    impl<T> AsRef<T> for Indexed<T> {
        fn as_ref(&self) -> &T {
            &self.value
        }
    }

    const X: i32 = 10;

    #[test]
    fn find_next() {
        let needle = &[1, 2, 3, 4];
        assert_eq_uut!(needle, &[]; insert_end(&1));
        assert_eq_uut!(needle, &[1]; insert_from(0, &2));
        assert_eq_uut!(needle, &[1, 2]; insert_from(0, &3));
        assert_eq_uut!(needle, &[1, 2, 3]; insert_from(0, &4));
        assert_eq_uut!(needle, &[1, 2, 3, 4]; matched(0));
        assert_eq_uut!(needle, &[X, 1, 2, 3, 4]; matched(1));
        assert_eq_uut!(needle, &[X, X, 1, 2, 3, 4]; matched(2));
        //                       0  1  2  3 [4]
        assert_eq_uut!(needle, &[1, 2, 3, 4, 1];
            matched_delete(0, &Indexed { index: 4, value: 1 })
        );
        //                       0  1 [2]
        assert_eq_uut!(needle, &[X, X, 1, 2]; insert_from(2, &3));
    }

    #[test]
    fn find_interspersed() {
        let needle = &[1, 2, 3];
        let x_at = |index| Indexed { index, value: X };
        assert_eq_uut!(needle, &[X, 1, X, 2]; matched_delete(1, &x_at(2)));
        assert_eq_uut!(needle, &[X, 1, X, 2, X]; matched_delete(1, &x_at(2)));
        assert_eq_uut!(needle, &[X, 1, 2, X]; matched_delete(1, &x_at(3)));
        assert_eq_uut!(needle, &[X, 1, 2, X, 3]; matched_delete(1, &x_at(3)));
        assert_eq_uut!(needle, &[X, 1, 2, 3, X]; matched_delete(1, &x_at(4)));
        assert_eq_uut!(needle, &[X, 1, 2, 3]; matched(1));
    }
}
