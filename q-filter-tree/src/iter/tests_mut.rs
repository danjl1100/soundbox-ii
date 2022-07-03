// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{
    id::{NodePathRefTyped, NodePathTyped},
    Tree,
};

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
            });
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
        $iter.with_all(|_, _, _| panic!("expected iter is empty for with_all"));
    };
}

#[test]
fn empty() {
    let mut t: Tree<(), ()> = Tree::default();
    let root = t.root_id();
    //
    let mut iter = t.enumerate_mut();
    assert_iter!(iter.with_next(
        filters = [()],
        path = &root,
        child_len = 0,
    ) as "root");
    assert_iter!(drop(iter));
}

#[test]
fn single() {
    let mut t: Tree<(), String> = Tree::default();
    let root = t.root_id();
    // \ root
    // |--  child1
    let mut root_ref = root.try_ref(&mut t);
    root_ref.filter = "this is root".to_string();
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    root.try_ref(&mut t).filter = "this is root".to_string();
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1's filter".to_string();
    //
    {
        let mut iter = t.enumerate_mut();
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
    let mut iter = t.enumerate_mut();
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
    let mut t: Tree<(), &str> = Tree::default();
    let root = t.root_id();
    // \ root
    // |--\ child1
    //    |--\ child2
    //       |-- child3
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    //
    let child1 = root_ref.add_child_default();
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    let mut child1_ref = child1_ref.child_nodes().expect("child1 is chain");
    let child2 = child1_ref.add_child_default();
    let mut child2_ref = child2.try_ref(&mut t).expect("child2 exists");
    let mut child2_ref = child2_ref.child_nodes().expect("child2 is chain");
    let child3 = child2_ref.add_child_default();
    root.try_ref(&mut t).filter = "the root";
    child1.try_ref(&mut t).expect("child1 exists").filter = "foo";
    child2.try_ref(&mut t).expect("child2 exists").filter = "bar";
    child3.try_ref(&mut t).expect("child3 exists").filter = "baz";
    //
    let mut iter = t.enumerate_mut();
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
    //
    let mut iter_child2 = t.enumerate_mut_subtree(&child2).expect("child2 exists");
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
    //
    let mut iter_child3 = t.enumerate_mut_subtree(&child3).expect("child3 exists");
    assert_iter!(iter_child3.with_next(
        filters = ["the root", "foo", "bar", "baz"],
        path = &child3,
        child_len = 0,
    ) as "child3");
    assert_iter!(drop(iter_child3));
}

#[test]
fn double() {
    let mut t: Tree<(), &str> = Tree::default();
    let root = t.root_id();
    // \ root
    // |--  child1
    // |--  child2
    let mut root_ref = root.try_ref(&mut t);
    root_ref.filter = "thorny root";
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    let child2 = root_ref.add_child_default();
    root.try_ref(&mut t).filter = "thorny root";
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1 carrot";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2 lemon";
    //
    let mut iter = t.enumerate_mut();
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
    //
    let mut iter_child1 = t.enumerate_mut_subtree(&child1).expect("child1 exists");
    assert_iter!(iter_child1.with_all([
        filters = ["thorny root", "child1 carrot"],
        path = &child1,
        child_len = 0,
    ]));
    //
    let mut iter_child2 = t.enumerate_mut_subtree(&child2).expect("child2 exists");
    assert_iter!(iter_child2.with_all([
        filters = ["thorny root", "child2 lemon"],
        path = &child2,
        child_len = 0,
    ]));
}

#[allow(clippy::too_many_lines)] // yikes...
#[test]
fn complex() {
    let mut t: Tree<(), &str> = Tree::new();
    let root = t.root_id();
    // \ root
    // |--\ base
    //    |--  child1
    //    |--  child2
    //    |--  child3
    //    |--\ child4
    //       |--  child4_child
    //    |--  child5
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let base = root_ref.add_child_default();
    let mut base_ref = base.try_ref(&mut t).expect("base exists");
    let mut base_ref = base_ref.child_nodes().expect("base is chain");
    let child1 = base_ref.add_child_default();
    let child2 = base_ref.add_child_default();
    let child3 = base_ref.add_child_default();
    let child4 = base_ref.add_child_default();
    let child5 = base_ref.add_child_default();
    let mut child4_ref = child4.try_ref(&mut t).expect("child4 exists");
    let mut child4_ref = child4_ref.child_nodes().expect("child4 is chain");
    let child4_child = child4_ref.add_child_default();
    root.try_ref(&mut t).filter = "root";
    base.try_ref(&mut t).expect("base exists").filter = "base";
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2";
    child3.try_ref(&mut t).expect("child3 exists").filter = "child3";
    child4.try_ref(&mut t).expect("child4 exists").filter = "child4";
    child5.try_ref(&mut t).expect("child5 exists").filter = "child5";
    child4_child
        .try_ref(&mut t)
        .expect("child4_child exists")
        .filter = "child4_child";
    // from ROOT
    let mut iter = t.enumerate_mut();
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
    // from BASE
    let mut iter_base = t.enumerate_mut_subtree(&base).expect("base exists");
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
    // from CHILD4
    let mut iter_child4 = t.enumerate_mut_subtree(&child4).expect("child4 exists");
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
    let mut t: Tree<(), &str> = Tree::new();
    let root = t.root_id();
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
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let base = root_ref.add_child_default();
    let mut base_ref = base.try_ref(&mut t).expect("base exists");
    let mut base_ref = base_ref.child_nodes().expect("base is chain");
    let child1 = base_ref.add_child_default();
    let child2 = base_ref.add_child_default();
    let child3 = base_ref.add_child_default();
    let child4 = base_ref.add_child_default();
    let child5 = base_ref.add_child_default();
    let mut child2_ref = child2.try_ref(&mut t).expect("child2 exists");
    let mut child2_ref = child2_ref.child_nodes().expect("child2 is chain");
    let child2_child = child2_ref.add_child_default();
    let child2_child2 = child2_ref.add_child_default();
    let mut child4_ref = child4.try_ref(&mut t).expect("child4 exists");
    let mut child4_ref = child4_ref.child_nodes().expect("child4 is chain");
    let child4_child = child4_ref.add_child_default();
    let mut child4_child_ref = child4_child.try_ref(&mut t).expect("child4_child exists");
    let mut child4_child_ref = child4_child_ref
        .child_nodes()
        .expect("child4_child is chain");
    let child4_child_child = child4_child_ref.add_child_default();
    root.try_ref(&mut t).filter = "root";
    base.try_ref(&mut t).expect("base exists").filter = "base";
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2";
    child3.try_ref(&mut t).expect("child3 exists").filter = "child3";
    child4.try_ref(&mut t).expect("child4 exists").filter = "child4";
    child5.try_ref(&mut t).expect("child5 exists").filter = "child5";
    child4_child
        .try_ref(&mut t)
        .expect("child4_child exists")
        .filter = "child4_child";
    //
    let mut iter = t.enumerate_mut();
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
    //
    let mut iter_child2 = t.enumerate_mut_subtree(&child2).expect("child2 exists");
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
    //
    let mut iter_child4 = t.enumerate_mut_subtree(&child4).expect("child4 exists");
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
    //
    let mut iter_child4_child = t
        .enumerate_mut_subtree(&child4_child)
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
    //
    let mut iter_child5 = t.enumerate_mut_subtree(&child5).expect("child5 exists");
    assert_iter!(iter_child5.with_all([
        filters = ["root", "base", "child5"],
        path = &child5,
        child_len = 0,
    ]));
}

#[test]
fn root_siblings() {
    let mut t: Tree<(), &str> = Tree::new();
    let root = t.root_id();
    // \ root
    // |-- child1
    // |-- child2
    // |-- child3
    // |-- child4
    let mut root_ref = root.try_ref(&mut t);
    root_ref.filter = "root";
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    let child2 = root_ref.add_child_default();
    let child3 = root_ref.add_child_default();
    let child4 = root_ref.add_child_default();
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2";
    child3.try_ref(&mut t).expect("child3 exists").filter = "child3";
    child4.try_ref(&mut t).expect("child4 exists").filter = "child4";
    //
    let mut iter = t.enumerate_mut();
    assert_iter!(iter.with_all(
        [filters = ["root"], path = &root, child_len = 4],
        [filters = ["root", "child1"], path = &child1, child_len = 0],
        [filters = ["root", "child2"], path = &child2, child_len = 0],
        [filters = ["root", "child3"], path = &child3, child_len = 0],
        [filters = ["root", "child4"], path = &child4, child_len = 0],
    ));
}
