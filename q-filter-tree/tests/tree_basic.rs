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
    let mut root_ref = t.get_mut(&root).expect("root exists");
    const N: usize = 10;
    for i in 0..N {
        root_ref.push_item(i);
    }
    for i in 0..N {
        assert_eq!(root_ref.pop_item(), Ok(i));
    }
    assert_eq!(
        root_ref.pop_item(),
        Err(PopError::Empty(root.clone().into()))
    );
    // filter
    let root_filter = root_ref.filter();
    assert_eq!(*root_filter, None);
    *root_filter = Some(String::from("my filter"));
    let mut root_ref = t.get_mut(&root).expect("root exists");
    assert_eq!(root_ref.filter(), &mut Some(String::from("my filter")));
}
#[test]
fn two_nodes() {
    let mut t = Tree::new();
    let root = t.root_id();
    //
    let mut root_ref = t.get_mut(&root).expect("root exists");
    let child = root_ref.add_child(None);
    // verify count
    assert_eq!(t.sum_node_count(), 2);
    // filter
    let mut child_ref = t.get_mut(&child).expect("child exists");
    *child_ref.filter() = Some(String::from("child_filter"));
    let mut root_ref = t.get_mut(&root).expect("root exists");
    *root_ref.filter() = Some(String::from("root_filter"));
    // item
    const N: usize = 5;
    for i in 0..N {
        t.get_mut(&child).expect("child exists").push_item(i);
        t.get_mut(&root).expect("root exists").push_item(i + 500);
    }
    for i in 0..N {
        assert_eq!(t.get_mut(&child).expect("child exists").pop_item(), Ok(i));
        assert_eq!(
            t.get_mut(&root).expect("root exists").pop_item(),
            Ok(i + 500)
        );
    }
    assert_eq!(
        t.get_mut(&child).expect("child exists").pop_item(),
        Err(PopError::Empty(child.into()))
    );
    assert_eq!(
        t.get_mut(&root).expect("root exists").pop_item(),
        Err(PopError::Blocked(root.into()))
    );
}
#[test]
fn node_pop_chain() {
    let mut t: Tree<_, ()> = Tree::new();
    let root = t.root_id();
    //
    let mut root_ref = t.get_mut(&root).expect("root exists");
    let child1 = root_ref.add_child(None);
    let mut child1_ref = t.get_mut(&child1).expect("child1 exists");
    let child2 = child1_ref.add_child(None);
    // verify count
    assert_eq!(t.sum_node_count(), 3);
    // fill child2
    let mut child2_ref = t.get_mut(&child2).expect("child2 exists");
    for i in 0..4 {
        child2_ref.push_item(i);
    }
    // verify child2 pop
    assert_eq!(child2_ref.pop_item(), Ok(0));
    assert_eq!(child2_ref.pop_item(), Ok(1));
    // verify child1 not popping
    let mut child1_ref = t.get_mut(&child1).expect("child1 exists");
    assert_eq!(
        child1_ref.pop_item(),
        Err(PopError::Blocked((*child1).clone()))
    );
    // allow child1 <- child2
    let mut child2_ref = t.get_child_mut(&child2).expect("child2 exists");
    child2_ref.set_weight(1);
    // verify child1 chain from child2
    let mut child1_ref = t.get_mut(&child1).expect("child2 exists");
    assert_eq!(child1_ref.pop_item(), Ok(2));
    assert_eq!(child1_ref.pop_item(), Ok(3));
    assert_eq!(child1_ref.pop_item(), Err(PopError::Empty(child1.into())));
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
    let mut root_ref = t.get_mut(&root).expect("root exists");
    let base = root_ref.add_child(None);
    let mut base_ref = t.get_mut(&base).expect("base exists");
    let child1 = base_ref.add_child(None);
    let child2 = base_ref.add_child(None);
    let child3 = base_ref.add_child(None);
    let child4 = base_ref.add_child(None);
    let child5 = base_ref.add_child(None);
    let mut child4_ref = t.get_mut(&child4).expect("child4 exists");
    let child4_child = child4_ref.add_child(None);
    // fill child4
    for i in 0..10 {
        child4_ref.push_item(i);
    }
    // verify count
    assert_eq!(t.sum_node_count(), 8);
    // verify root pop
    t.get_child_mut(&base).expect("base exists").set_weight(1);
    t.get_child_mut(&child4)
        .expect("child4 exists")
        .set_weight(1);
    let mut root_ref = t.get_mut(&root).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok(0));
    assert_eq!(root_ref.pop_item(), Ok(1));
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
    let mut root_ref = t.get_mut(&root).expect("root exists");
    assert_eq!(root_ref.pop_item(), Err(PopError::Blocked(root.into())));
}
