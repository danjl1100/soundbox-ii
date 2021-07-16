use q_filter_tree::{
    error::{PopError, RemoveError},
    Tree,
};
#[test]
fn creates_single() {
    let mut t = Tree::new();
    let root = t.root_id();
    // verify count
    assert_eq!(t.sum_node_count(), 1);
    // item
    const N: usize = 10;
    for i in 0..N {
        t.push_item(&root, i).expect("root exists");
    }
    for i in 0..N {
        assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(i));
    }
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Err(PopError::Empty(root.clone().into()))
    );
    // filter
    assert_eq!(t.get_filter(&root).expect("root exists"), None);
    t.set_filter(&root, String::from("my root"))
        .expect("root exists");
    assert_eq!(
        t.get_filter(&root).expect("root exists"),
        Some(&String::from("my root"))
    );
}
#[test]
fn two_nodes() {
    let mut t = Tree::new();
    let root = t.root_id();
    //
    let child = t.add_child(&root, None).expect("root exists");
    // verify count
    assert_eq!(t.sum_node_count(), 2);
    // filter
    t.set_filter(&child, String::from("child_filter"))
        .expect("child exists");
    t.set_filter(&root, String::from("root_filter"))
        .expect("root exists");
    // item
    const N: usize = 5;
    for i in 0..N {
        t.push_item(&child, i).expect("child exists");
        t.push_item(&root, i + 500).expect("root exists");
    }
    for i in 0..N {
        assert_eq!(t.pop_item_from(&child).expect("child exists"), Ok(i));
        assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(i + 500));
    }
    assert_eq!(
        t.pop_item_from(&child).expect("child exists"),
        Err(PopError::Empty(child.into()))
    );
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Err(PopError::Blocked(root.into()))
    );
}
#[test]
fn node_pop_chain() {
    let mut t: Tree<_, ()> = Tree::new();
    let root = t.root_id();
    //
    let child1 = t.add_child(&root, None).expect("root exists");
    let child2 = t.add_child(&child1, None).expect("child1 exists");
    // verify count
    assert_eq!(t.sum_node_count(), 3);
    // fill child2
    for i in 0..4 {
        t.push_item(&child2, i).expect("child2 exists");
    }
    // verify child2 pop
    assert_eq!(t.pop_item_from(&child2).expect("child2 exists"), Ok(0));
    assert_eq!(t.pop_item_from(&child2).expect("child2 exists"), Ok(1));
    // verify child1 not popping
    assert_eq!(
        t.pop_item_from(&child1).expect("child2 exists"),
        Err(PopError::Blocked((*child1).clone()))
    );
    // allow child1 <- child2
    t.set_weight(&child2, 1).expect("child2 exists");
    // verify child1 chain from child2
    assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(2));
    assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(3));
    assert_eq!(
        t.pop_item_from(&child1).expect("child2 exists"),
        Err(PopError::Empty(child1.into()))
    );
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
    let base = t.add_child(&root, None).expect("root exists");
    let child1 = t.add_child(&base, None).expect("base exists");
    let child2 = t.add_child(&base, None).expect("base exists");
    let child3 = t.add_child(&base, None).expect("base exists");
    let child4 = t.add_child(&base, None).expect("base exists");
    let child5 = t.add_child(&base, None).expect("base exists");
    let child4_child = t.add_child(&child4, None).expect("child4 exists");
    // fill child4
    for i in 0..10 {
        t.push_item(&child4, i).expect("child4 exists");
    }
    // verify count
    assert_eq!(t.sum_node_count(), 8);
    // verify root pop
    t.set_weight(&base, 1).expect("base exists");
    t.set_weight(&child4, 1).expect("child4 exists");
    assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(0));
    assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(1));
    // fails - remove root
    assert_eq!(
        t.remove_node(&root),
        Err(RemoveError::Root(root.clone().into()))
    );
    // fails - remove base
    assert_eq!(
        t.remove_node(&base),
        Err(RemoveError::NonEmpty(
            base.clone().into(),
            vec![
                child1.clone().into(),
                child2.clone().into(),
                child3.clone().into(),
                child4.clone().into(),
                child5.clone().into(),
            ]
        ))
    );
    // fails - remove child4
    assert_eq!(
        t.remove_node(&child4),
        Err(RemoveError::NonEmpty(
            child4.clone().into(),
            vec![child4_child.clone().into()]
        ))
    );
    // success - remove child4_child, then child4
    assert_eq!(t.remove_node(&child4_child), Ok(()));
    assert_eq!(t.remove_node(&child4), Ok(()));
    // fails - remove child4 AGAIN
    assert_eq!(
        t.remove_node(&child4),
        Err(RemoveError::SequenceMismatch(child4, child5.sequence()))
    );
    // verify root pop empty
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Err(PopError::Blocked(root.into()))
    );
}
