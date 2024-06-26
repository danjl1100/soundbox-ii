// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{
    id::{ty, NodeId},
    Tree,
};

type RootId = NodeId<ty::Root>;
type ChildId = NodeId<ty::Child>;

pub(super) fn create_empty() -> (Tree<(), ()>, NodeId<ty::Root>) {
    let t = Tree::default();
    let root = t.root_id();
    (t, root)
}
#[test]
fn empty() {
    let (t, root) = create_empty();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()));
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

pub(super) fn create_single() -> (Tree<(), String>, RootId, ChildId) {
    let mut t = Tree::default();
    let root = t.root_id();
    // \ root
    // |--  child1
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    root.try_ref(&mut t).filter = "this is root".to_string();
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1's filter".to_string();
    (t, root, child1)
}
#[test]
fn single() {
    let (t, root, child1) = create_single();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

pub(super) fn create_single_line() -> (Tree<(), &'static str>, RootId, ChildId, ChildId, ChildId) {
    let mut t = Tree::default();
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
    (t, root, child1, child2, child3)
}
#[test]
fn single_line() {
    let (t, root, child1, child2, child3) = create_single_line();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    assert_eq!(iter.next(), Some(child2.into()), "child2");
    assert_eq!(iter.next(), Some(child3.into()), "child3");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

pub(super) fn create_double() -> (Tree<(), &'static str>, RootId, ChildId, ChildId) {
    let mut t = Tree::default();
    let root = t.root_id();
    // \ root
    // |--  child1
    // |--  child2
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    let child2 = root_ref.add_child_default();
    root.try_ref(&mut t).filter = "thorny root";
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1 carrot";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2 lemon";
    (t, root, child1, child2)
}
#[test]
fn double() {
    let (t, root, child1, child2) = create_double();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    assert_eq!(iter.next(), Some(child2.into()), "child2");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn create_complex() -> (
    Tree<(), &'static str>,
    RootId,
    ChildId,
    ChildId,
    ChildId,
    ChildId,
    (ChildId, (ChildId,)),
    ChildId,
) {
    let mut t = Tree::new();
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
    (
        t,
        root,
        base,
        child1,
        child2,
        child3,
        (child4, (child4_child,)),
        child5,
    )
}
#[test]
fn complex() {
    let (t, root, base, child1, child2, child3, (child4, (child4_child,)), child5) =
        create_complex();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(base.into()), "base");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    assert_eq!(iter.next(), Some(child2.into()), "child2");
    assert_eq!(iter.next(), Some(child3.into()), "child3");
    assert_eq!(iter.next(), Some(child4.into()), "child4");
    assert_eq!(iter.next(), Some(child4_child.into()), "child4_child");
    assert_eq!(iter.next(), Some(child5.into()), "child5");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn create_complex2() -> (
    Tree<(), &'static str>,
    RootId,
    ChildId,
    ChildId,
    (ChildId, (ChildId, ChildId)),
    ChildId,
    (ChildId, (ChildId, (ChildId,))),
    ChildId,
) {
    let mut t = Tree::new();
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
    (
        t,
        root,
        base,
        child1,
        (child2, (child2_child, child2_child2)),
        child3,
        (child4, (child4_child, (child4_child_child,))),
        child5,
    )
}
#[test]
fn complex2() {
    let (
        t,
        root,
        base,
        child1,
        (child2, (child2_child, child2_child2)),
        child3,
        (child4, (child4_child, (child4_child_child,))),
        child5,
    ) = create_complex2();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(base.into()), "base");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    assert_eq!(iter.next(), Some(child2.into()), "child2");
    assert_eq!(iter.next(), Some(child2_child.into()), "child2_child");
    assert_eq!(iter.next(), Some(child2_child2.into()), "child2_child2");
    assert_eq!(iter.next(), Some(child3.into()), "child3");
    assert_eq!(iter.next(), Some(child4.into()), "child4");
    assert_eq!(iter.next(), Some(child4_child.into()), "child4_child");
    assert_eq!(
        iter.next(),
        Some(child4_child_child.into()),
        "child4_child_child"
    );
    assert_eq!(iter.next(), Some(child5.into()), "child5");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn create_root_siblings() -> (
    Tree<(), &'static str>,
    RootId,
    (ChildId, ChildId, ChildId, ChildId),
) {
    let mut t = Tree::new();
    let root = t.root_id();
    // \ root
    // |-- child1
    // |-- child2
    // |-- child3
    // |-- child4
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child_default();
    let child2 = root_ref.add_child_default();
    let child3 = root_ref.add_child_default();
    let child4 = root_ref.add_child_default();
    root.try_ref(&mut t).filter = "root";
    child1.try_ref(&mut t).expect("child1 exists").filter = "child1";
    child2.try_ref(&mut t).expect("child2 exists").filter = "child2";
    child3.try_ref(&mut t).expect("child3 exists").filter = "child3";
    child4.try_ref(&mut t).expect("child4 exists").filter = "child4";
    (t, root, (child1, child2, child3, child4))
}
#[test]
fn root_siblings() {
    let (t, root, (child1, child2, child3, child4)) = create_root_siblings();
    let mut iter = t.iter_ids();
    assert_eq!(iter.next(), Some(root.into()), "root");
    assert_eq!(iter.next(), Some(child1.into()), "child1");
    assert_eq!(iter.next(), Some(child2.into()), "child2");
    assert_eq!(iter.next(), Some(child3.into()), "child3");
    assert_eq!(iter.next(), Some(child4.into()), "child4");
    for _ in 0..20 {
        assert_eq!(iter.next(), None);
    }
}
