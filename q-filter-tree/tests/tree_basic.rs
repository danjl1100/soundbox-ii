// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::{borrow::Cow, collections::VecDeque, iter::FromIterator};

use q_filter_tree::{error::RemoveError, NodeInfo, OrderType, Tree};
#[test]
fn creates_single() {
    let mut t = Tree::new();
    let root = t.root_id();
    // verify count
    assert_eq!(t.sum_node_count(), 1);
    // item
    let mut root_ref = root.try_ref(&mut t);
    const N: usize = 10;
    for i in 0..N {
        root_ref.push_item(i);
    }
    for i in 0..N {
        assert_eq!(root_ref.pop_item(), Some(Cow::Owned(i)));
    }
    assert_eq!(root_ref.pop_item(), None);
    // filter
    let root_filter = &mut root_ref.filter;
    assert_eq!(*root_filter, None);
    *root_filter = Some(String::from("my filter"));
    let root_ref = root.try_ref(&mut t);
    assert_eq!(&root_ref.filter, &Some(String::from("my filter")));
}
#[test]
fn two_nodes() {
    let mut t = Tree::new();
    let root = t.root_id();
    //
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child = root_ref.add_child_default();
    // verify count
    assert_eq!(t.sum_node_count(), 2);
    // filter
    let mut child_ref = child.try_ref(&mut t).expect("child exists");
    child_ref.filter = Some(String::from("child_filter"));
    let mut root_ref = root.try_ref(&mut t);
    root_ref.filter = Some(String::from("root_filter"));
    // item
    const N: usize = 5;
    for i in 0..N {
        child.try_ref(&mut t).expect("child exists").push_item(i);
        root.try_ref(&mut t).push_item(i + 500);
    }
    for i in 0..N {
        assert_eq!(
            child.try_ref(&mut t).expect("child exists").pop_item(),
            Some(Cow::Owned(i))
        );
        assert_eq!(root.try_ref(&mut t).pop_item(), Some(Cow::Owned(i + 500)));
    }
    assert_eq!(
        child.try_ref(&mut t).expect("child exists").pop_item(),
        None
    );
    assert_eq!(root.try_ref(&mut t).pop_item(), None);
}
#[test]
fn node_pop_chain() {
    let mut t: Tree<_, ()> = Tree::new();
    let root = t.root_id();
    //
    let mut root_ref = root.try_ref(&mut t);
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child1 = root_ref.add_child(0);
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    let mut child1_ref = child1_ref.child_nodes().expect("child1 is chain");
    let child2 = child1_ref.add_child(0);
    // verify count
    assert_eq!(t.sum_node_count(), 3);
    // fill child2
    let mut child2_ref = child2.try_ref(&mut t).expect("child2 exists");
    for i in 0..4 {
        child2_ref.push_item(i);
    }
    // verify child2 pop
    assert_eq!(child2_ref.pop_item(), Some(Cow::Owned(0)));
    assert_eq!(child2_ref.pop_item(), Some(Cow::Owned(1)));
    // verify child1 not popping
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    assert_eq!(child1_ref.pop_item(), None);
    // allow child1 <- child2
    let mut child2_ref = child2.try_ref(&mut t).expect("child2 exists");
    child2_ref.set_weight(1);
    // verify child1 chain from child2
    let mut child1_ref = child1.try_ref(&mut t).expect("child2 exists");
    assert_eq!(child1_ref.pop_item(), Some(Cow::Owned(2)));
    assert_eq!(child1_ref.pop_item(), Some(Cow::Owned(3)));
    assert_eq!(child1_ref.pop_item(), None);
}
#[test]
fn node_removal() {
    let mut t: Tree<_, ()> = Tree::new();
    // verify count
    assert_eq!(t.sum_node_count(), 1);
    //
    let root = t.root_id();
    // \ root
    // ---\ base
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
    let _child1 = base_ref.add_child_default();
    let _child2 = base_ref.add_child_default();
    let _child3 = base_ref.add_child_default();
    let child4 = base_ref.add_child_default();
    let child5 = base_ref.add_child_default();
    let mut child4_ref = child4.try_ref(&mut t).expect("child4 exists");
    let mut child4_refc = child4_ref.child_nodes().expect("child4 is chain");
    let child4_child = child4_refc.add_child_default();
    // fill child4
    for i in 0..10 {
        child4_ref.push_item(i);
    }
    // verify count
    assert_eq!(t.sum_node_count(), 8);
    // verify root pop
    base.try_ref(&mut t).expect("base exists").set_weight(1);
    child4.try_ref(&mut t).expect("child4 exists").set_weight(1);
    let mut root_ref = root.try_ref(&mut t);
    assert_eq!(root_ref.pop_item(), Some(Cow::Owned(0)));
    assert_eq!(root_ref.pop_item(), Some(Cow::Owned(1)));
    // this is enforced by the compiler, now!
    // // fails - remove root
    // assert_eq!(
    //     t.remove_node(&root),
    //     Err(RemoveError::Root(root.clone().into()))
    // );
    // fails - remove base
    assert_eq!(
        t.remove_node(&base),
        Ok(Err(RemoveError::NonEmpty(
            base.clone(),
            // vec![
            //     child1.clone().into(),
            //     child2.clone().into(),
            //     child3.clone().into(),
            //     child4.clone().into(),
            //     child5.clone().into(),
            // ]
        )))
    );
    // fails - remove child4
    assert_eq!(
        t.remove_node(&child4),
        Ok(Err(RemoveError::NonEmpty(
            child4.clone(),
            // vec![child4_child.clone().into()]
        )))
    );
    // success - remove child4_child, then child4
    assert_eq!(
        t.remove_node(&child4_child),
        Ok(Ok((
            1,
            NodeInfo::Chain {
                filter: None,
                order: OrderType::InOrder,
                queue: VecDeque::new(),
            }
        )))
    );
    assert_eq!(
        t.remove_node(&child4),
        Ok(Ok((
            1,
            NodeInfo::Chain {
                filter: None,
                order: OrderType::InOrder,
                queue: VecDeque::from_iter(2..10),
            }
        )))
    );
    // fails - remove child4 AGAIN
    assert_eq!(
        t.remove_node(&child4),
        Ok(Err(RemoveError::SequenceMismatch(
            child4,
            child5.sequence()
        )))
    );
    // verify root pop empty
    let mut root_ref = root.try_ref(&mut t);
    assert_eq!(root_ref.pop_item(), None);
}
