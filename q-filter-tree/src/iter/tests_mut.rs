// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::IterMutBreadcrumb;
use crate::id::{NodePathRefTyped, NodePathTyped};
use shared::{IgnoreNever, Never};

#[derive(Debug, PartialEq, Eq)]
struct SentinelResult(&'static str);

macro_rules! assert_iter {
    (
        $iter:ident.with_next(
            filters = [ $($filter:expr),+ $(,)? ],
            path = $path:expr,
            child_len = $child_len:expr$(,)?
        )
        as $sentinel:expr
    ) => {
        let result = $iter.with_next(|filters, path, node_ref_mut| {
            assert_eq!(filters, vec![ $($filter),+ ]);
            assert_eq!(path, NodePathRefTyped::from($path));
            assert_eq!(node_ref_mut.child_nodes_len(), $child_len);
            SentinelResult($sentinel)
        });
        assert_eq!(result, Some(SentinelResult($sentinel)));
    };
    (
        $iter:ident.with_all(
            $( [
               filters = [ $($filter:expr),+ $(,)? ],
               path = $path:expr,
               child_len = $child_len:expr $(,)?
            ]),* $(,)?
        )
    ) => {
        {
            let mut filter_sets = vec![];
            let mut paths: Vec<NodePathTyped> = vec![];
            let mut child_lens = vec![];
            $iter.with_all(|filters, path, node_ref_mut| {
                filter_sets.push(Vec::from(filters));
                paths.push(path.clone_inner());
                child_lens.push(node_ref_mut.child_nodes_len());
                Ok::<_, Never>(())
            }).ignore_never();
            assert_eq!(filter_sets, vec![
                $(
                    vec![ $($filter),+ ]
                ),*
            ]);
            assert_eq!(paths, vec![ $(NodePathTyped::from($path.clone())),* ]);
            assert_eq!(child_lens, vec![ $($child_len),* ]);
        }
    };
    ( drop($iter:ident) ) => {
        {
            assert_iter!($iter.with_next(empty));
            assert_iter!($iter.with_all(empty));
            drop($iter);
        }
    };
    ( $iter:ident.with_next(empty) ) => {
        assert_eq!(
            $iter.with_next(|_, _, _| panic!("expected iter is empty for with_next")),
            None
        );
    };
    ( $iter:ident.with_all(empty) ) => {
        $iter.with_all::<_, shared::Never>(|_, _, _| panic!("expected iter is empty for with_all")).ignore_never();
    };
}

#[test]
fn empty() {
    let (mut t, root) = super::tests::create_empty();
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_next(
        filters = [()],
        path = &root,
        child_len = 0,
    ) as "root");
    assert_iter!(drop(iter));
}

#[test]
fn single() {
    let (mut t, root, child1) = super::tests::create_single();
    // \ root
    // |--  child1
    {
        let mut iter = t.enumerate_mut_filters();
        assert_iter!(iter.with_next(
            filters = ["this is root".to_string()],
            path = &root,
            child_len = 1,
        ) as "root");
        assert_iter!(iter.with_next(
            filters = ["this is root".to_string(), "child1's filter".to_string()],
            path = &child1,
            child_len = 0,
        ) as "child1");
        assert_iter!(drop(iter));
    }
    //
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [
            filters = ["this is root".to_string()],
            path = &root,
            child_len = 1,
        ],
        [
            filters = ["this is root".to_string(), "child1's filter".to_string()],
            path = &child1,
            child_len = 0,
        ]
    ));
}

#[test]
fn single_line() {
    let (mut t, root, child1, child2, child3) = super::tests::create_single_line();
    // \ root
    // |--\ child1
    //    |--\ child2
    //       |-- child3
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [
            filters = ["the root"], //
            path = &root,
            child_len = 1,
        ],
        [
            filters = ["the root", "foo"], //
            path = &child1,
            child_len = 1,
        ],
        [
            filters = ["the root", "foo", "bar"],
            path = &child2,
            child_len = 1,
        ],
        [
            filters = ["the root", "foo", "bar", "baz"],
            path = &child3,
            child_len = 0,
        ]
    ));
    drop(iter); // TODO why is this needed?

    //
    let mut iter_child2 = t
        .enumerate_mut_subtree_filters(&child2)
        .expect("child2 exists");
    assert_iter!(iter_child2.with_all(
        [
            filters = ["the root", "foo", "bar"],
            path = &child2,
            child_len = 1,
        ],
        [
            filters = ["the root", "foo", "bar", "baz"],
            path = &child3,
            child_len = 0,
        ]
    ));
    drop(iter_child2); // TODO why is this needed?

    //
    let mut iter_child3 = t
        .enumerate_mut_subtree_filters(&child3)
        .expect("child3 exists");
    assert_iter!(iter_child3.with_next(
        filters = ["the root", "foo", "bar", "baz"],
        path = &child3,
        child_len = 0,
    ) as "child3");
    assert_iter!(drop(iter_child3));
}

#[test]
fn double() {
    let (mut t, root, child1, child2) = super::tests::create_double();
    // \ root
    // |--  child1
    // |--  child2
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [
            filters = ["thorny root"], //
            path = &root,
            child_len = 2,
        ],
        [
            filters = ["thorny root", "child1 carrot"],
            path = &child1,
            child_len = 0,
        ],
        [
            filters = ["thorny root", "child2 lemon"],
            path = &child2,
            child_len = 0,
        ],
    ));
    drop(iter); // TODO why is this needed?

    //
    let mut iter_child1 = t
        .enumerate_mut_subtree_filters(&child1)
        .expect("child1 exists");
    assert_iter!(iter_child1.with_all([
        filters = ["thorny root", "child1 carrot"],
        path = &child1,
        child_len = 0,
    ]));
    drop(iter_child1); // TODO why is this needed?

    //
    let mut iter_child2 = t
        .enumerate_mut_subtree_filters(&child2)
        .expect("child2 exists");
    assert_iter!(iter_child2.with_all([
        filters = ["thorny root", "child2 lemon"],
        path = &child2,
        child_len = 0,
    ]));
}

#[test]
fn complex() {
    let (mut t, root, base, child1, child2, child3, (child4, (child4_child,)), child5) =
        super::tests::create_complex();
    // \ root
    // |--\ base
    //    |--  child1
    //    |--  child2
    //    |--  child3
    //    |--\ child4
    //       |--  child4_child
    //    |--  child5
    // from ROOT
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [
            filters = ["root"], //
            path = &root,
            child_len = 1,
        ],
        [
            filters = ["root", "base"], //
            path = &base,
            child_len = 5,
        ],
        [
            filters = ["root", "base", "child1"],
            path = &child1,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child2"],
            path = &child2,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child3"],
            path = &child3,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child4"],
            path = &child4,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child5"],
            path = &child5,
            child_len = 0,
        ],
    ));
    drop(iter); // TODO why is this needed?

    // from BASE
    let mut iter_base = t.enumerate_mut_subtree_filters(&base).expect("base exists");
    assert_iter!(iter_base.with_all(
        [
            filters = ["root", "base"], //
            path = &base,
            child_len = 5,
        ],
        [
            filters = ["root", "base", "child1"],
            path = &child1,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child2"],
            path = &child2,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child3"],
            path = &child3,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child4"],
            path = &child4,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child5"],
            path = &child5,
            child_len = 0,
        ],
    ));
    drop(iter_base); // TODO why is this needed?

    // from CHILD4
    let mut iter_child4 = t
        .enumerate_mut_subtree_filters(&child4)
        .expect("child4 exists");
    assert_iter!(iter_child4.with_all(
        [
            filters = ["root", "base", "child4"],
            path = &child4,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 0,
        ],
    ));
}

#[allow(clippy::too_many_lines)] // yikes...
#[test]
fn complex2() {
    let (
        mut t,
        root,
        base,
        child1,
        (child2, (child2_child, child2_child2)),
        child3,
        (child4, (child4_child, (child4_child_child,))),
        child5,
    ) = super::tests::create_complex2();
    // \ root
    // |--\ base
    //    |--  child1
    //    |--\ child2
    //       |-- child2_child
    //       |-- child2_child2
    //    |--  child3
    //    |--\ child4
    //       |--\ child4_child
    //          |--  chil4_child_child
    //    |--  child5
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [
            filters = ["root"], //
            path = &root,
            child_len = 1,
        ],
        [
            filters = ["root", "base"], //
            path = &base,
            child_len = 5,
        ],
        [
            filters = ["root", "base", "child1"],
            path = &child1,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child2"],
            path = &child2,
            child_len = 2,
        ],
        [
            filters = ["root", "base", "child2", ""],
            path = &child2_child,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child2", ""],
            path = &child2_child2,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child3"],
            path = &child3,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child4"],
            path = &child4,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child", ""],
            path = &child4_child_child,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child5"],
            path = &child5,
            child_len = 0,
        ],
    ));
    drop(iter); // TODO why is this needed?

    //
    let mut iter_child2 = t
        .enumerate_mut_subtree_filters(&child2)
        .expect("child2 exists");
    assert_iter!(iter_child2.with_all(
        [
            filters = ["root", "base", "child2"],
            path = &child2,
            child_len = 2,
        ],
        [
            filters = ["root", "base", "child2", ""],
            path = &child2_child,
            child_len = 0,
        ],
        [
            filters = ["root", "base", "child2", ""],
            path = &child2_child2,
            child_len = 0,
        ],
    ));
    drop(iter_child2); // TODO why is this needed?

    //
    let mut iter_child4 = t
        .enumerate_mut_subtree_filters(&child4)
        .expect("child4 exists");
    assert_iter!(iter_child4.with_all(
        [
            filters = ["root", "base", "child4"],
            path = &child4,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child", ""],
            path = &child4_child_child,
            child_len = 0,
        ],
    ));
    drop(iter_child4); // TODO why is this needed?

    //
    let mut iter_child4_child = t
        .enumerate_mut_subtree_filters(&child4_child)
        .expect("child4_child exists");
    assert_iter!(iter_child4_child.with_all(
        [
            filters = ["root", "base", "child4", "child4_child"],
            path = &child4_child,
            child_len = 1,
        ],
        [
            filters = ["root", "base", "child4", "child4_child", ""],
            path = &child4_child_child,
            child_len = 0,
        ],
    ));
    drop(iter_child4_child); // TODO why is this needed?

    //
    let mut iter_child5 = t
        .enumerate_mut_subtree_filters(&child5)
        .expect("child5 exists");
    assert_iter!(iter_child5.with_all([
        filters = ["root", "base", "child5"],
        path = &child5,
        child_len = 0,
    ]));
}

#[test]
fn root_siblings() {
    let (mut t, root, (child1, child2, child3, child4)) = super::tests::create_root_siblings();
    // \ root
    // |-- child1
    // |-- child2
    // |-- child3
    // |-- child4
    //
    let mut iter = t.enumerate_mut_filters();
    assert_iter!(iter.with_all(
        [filters = ["root"], path = &root, child_len = 4],
        [filters = ["root", "child1"], path = &child1, child_len = 0],
        [filters = ["root", "child2"], path = &child2, child_len = 0],
        [filters = ["root", "child3"], path = &child3, child_len = 0],
        [filters = ["root", "child4"], path = &child4, child_len = 0],
    ));
}
