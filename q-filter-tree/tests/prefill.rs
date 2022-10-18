// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use q_filter_tree::{SequenceAndItem, Tree};

fn assert_queue<'a, T, I>(queue_iter: I, expected: Vec<T>)
where
    T: Clone + PartialEq + 'a + std::fmt::Debug,
    I: Iterator<Item = &'a T>,
{
    let queue: Vec<T> = queue_iter.cloned().collect();
    assert_eq!(queue, expected);
}

#[test]
fn prefills_root() {
    let mut t: Tree<_, ()> = Tree::new();
    let root_id = t.root_id();
    let mut root_ref = root_id.try_ref(&mut t);
    let mut child_nodes = root_ref.child_nodes().expect("root is chain");
    let child1 = child_nodes.add_child_default();
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    child1_ref.overwrite_child_items_uniform(0..10);
    //
    let mut root_ref = root_id.try_ref(&mut t);
    assert_eq!(root_ref.queue_len(), 0);
    //
    root_ref.set_queue_prefill_len(5);
    assert_eq!(root_ref.queue_len(), 5);
    assert_queue(
        root_ref.queue_iter(),
        (0..5).map(SequenceAndItem::new_fn(1)).collect(),
    );
}

#[test]
fn prefills_chain_node() {
    let mut t: Tree<_, ()> = Tree::new();
    let root_id = t.root_id();
    let mut root_ref = root_id.try_ref(&mut t);
    let mut child_nodes = root_ref.child_nodes().expect("root is chain");
    let child1 = child_nodes.add_child_default();
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    let child2 = child1_ref
        .child_nodes()
        .expect("child1 is chain")
        .add_child_default();
    let mut child2_ref = child2.try_ref(&mut t).expect("child2 exists");
    child2_ref.overwrite_child_items_uniform(0..100);
    //
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    assert_eq!(child1_ref.queue_len(), 0);
    //
    child1_ref.set_queue_prefill_len(7);
    assert_eq!(child1_ref.queue_len(), 7);
    assert_queue(
        child1_ref.queue_iter(),
        (0..7).map(SequenceAndItem::new_fn(2)).collect(),
    );
}

#[test]
fn prefills_item_node() {
    let mut t: Tree<_, ()> = Tree::new();
    let root_id = t.root_id();
    let mut root_ref = root_id.try_ref(&mut t);
    let mut child_nodes = root_ref.child_nodes().expect("root is chain");
    let child1 = child_nodes.add_child_default();
    let mut child1_ref = child1.try_ref(&mut t).expect("child1 exists");
    child1_ref.overwrite_child_items_uniform(50..99);
    //
    assert_eq!(child1_ref.queue_len(), 0);
    //
    child1_ref.set_queue_prefill_len(20);
    assert_eq!(child1_ref.queue_len(), 20);
    assert_queue(
        child1_ref.queue_iter(),
        (50..70).map(SequenceAndItem::new_fn(1)).collect(),
    );
}
