// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use q_filter_tree::{
    id::{ty, NodeId, NodeIdTyped, NodePath, NodePathTyped},
    NodeInfo, Tree,
};
use serde_json::Result;

const EMPTY_NODE: &str = r#"[[],{"queue":[],"filter":null,"order":"InOrder"}]"#;
const ONE_CHILD: &str = r#"[[0],{"queue":[],"filter":null,"order":"InOrder"}]"#;
const FIVE_CHILD: &str = r#"[[0,0,0,0,0],{"queue":[],"filter":null,"order":"InOrder"}]"#;
#[test]
fn simple_serialize() -> Result<()> {
    let mut t: Tree<(), ()> = Tree::new();
    let root = t.root_id();
    //
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let _child = root_ref.add_child(0);
    //
    let json_str = serde_json::to_string(&t)?;
    assert_eq!(
        json_str,
        format!(
            r#"{{".#0":{ONE},".0#1":{EMPTY}}}"#,
            EMPTY = EMPTY_NODE,
            ONE = ONE_CHILD
        )
    );
    Ok(())
}

#[test]
fn complex_serialize() -> Result<()> {
    let mut t: Tree<(), ()> = Tree::new();
    let root = t.root_id();
    // \ root
    // ---\ base
    //    |--  child1
    //    |--  child2
    //    |--  child3
    //    |--\ child4
    //       |--  child4_child
    //    |--  child5
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let base = root_ref.add_child(0);
    let mut base_ref = base.try_ref(&mut t).expect("base exists");
    let mut base_ref = base_ref.child_nodes().expect("base is chain");
    let _child1 = base_ref.add_child(0);
    let _child2 = base_ref.add_child(0);
    let _child3 = base_ref.add_child(0);
    let child4 = base_ref.add_child(0);
    let _child5 = base_ref.add_child(0);
    let mut child4_ref = child4.try_ref(&mut t).expect("child4 exists");
    let mut child4_ref = child4_ref.child_nodes().expect("child4 is chain");
    let _child4_child = child4_ref.add_child(0);
    //
    let json_str = serde_json::to_string(&t)?;
    assert_eq!(
        json_str,
        format!(
            r#"{{".#0":{ONE},".0#1":{FIVE},".0.0#2":{EMPTY},".0.1#3":{EMPTY},".0.2#4":{EMPTY},".0.3#5":{ONE},".0.3.0#7":{EMPTY},".0.4#6":{EMPTY}}}"#,
            EMPTY = EMPTY_NODE,
            ONE = ONE_CHILD,
            FIVE = FIVE_CHILD
        )
    );
    Ok(())
}

#[test]
fn simple_deserialize() -> Result<()> {
    let tree_json = r#"
        {
          ".#0": [
            [ 1 ],
            {
              "queue": [],
              "filter": null,
              "order": "InOrder"
            }
          ],
          ".0#0": [
            [ 0 ],
            {
              "queue": [],
              "filter": null,
              "order": "InOrder"
            }
          ],
          ".0.0#0": [
            [],
            {
              "queue": ["Alfalfa", "Oats"],
              "filter": null,
              "order": "InOrder"
            }
          ]
        }"#;
    let mut t: Tree<String, ()> = serde_json::from_str(tree_json)?;
    //
    println!(
        "input:\n\t{}\ndeserialized to:\n\t{}",
        tree_json,
        serde_json::to_string(&t)?
    );
    //
    let root = t.root_id();
    assert_eq!(
        root.try_ref(&mut t).expect("root exists").pop_item_queued(),
        None
    );
    let child = unwrap_child_path(serde_json::from_str("\".0.0\"")?);
    let mut child_ref = child.try_ref(&mut t).expect("child exists");
    child_ref.set_weight(1);
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    assert_eq!(root_ref.pop_item_queued(), Some(String::from("Alfalfa")));
    assert_eq!(root_ref.pop_item_queued(), Some(String::from("Oats")));
    assert_eq!(root_ref.pop_item_queued(), None);
    Ok(())
}

fn unwrap_child_path(typed: NodePathTyped) -> NodePath<ty::Child> {
    match typed {
        NodePathTyped::Child(path) => path,
        NodePathTyped::Root(_) => {
            panic!("incorrect type on parsed NodePathTyped: {:#?}", typed)
        }
    }
}
fn unwrap_child_id(typed: NodeIdTyped) -> NodeId<ty::Child> {
    match typed {
        NodeIdTyped::Child(id) => id,
        NodeIdTyped::Root(_) => {
            panic!("incorrect type on parsed NodeIdTyped: {:#?}", typed)
        }
    }
}

#[test]
fn complex_deserialize() -> Result<()> {
    let tree_json = r#"
    {
      ".#0": [ [0], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0#1": [ [0,0,0,0,0], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.0#2": [ [], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.1#3": [ [], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.2#4": [ [], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.3#5": [ [0], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.3.0#6": [ [], {
        "queue": [],
        "filter": null,
        "order": "InOrder"
      }],
      ".0.4#7": [ [], {
        "items": ["ping", "pong"],
        "filter": null,
        "order": "InOrder"
      }]
    }"#;
    let mut t: Tree<&str, ()> = serde_json::from_str(tree_json)?;
    //
    println!(
        "input:\n\t{}\ndeserialized to:\n\t{}",
        tree_json,
        serde_json::to_string(&t)?
    );
    //
    let root = t.root_id();
    assert_eq!(
        root.try_ref(&mut t).expect("root exists").pop_item_queued(),
        None
    );
    const CHILD_PATH_STRS: &[&str] = &[
        "\".0\"",
        "\".0.0\"",
        "\".0.1\"",
        "\".0.2\"",
        "\".0.3\"",
        "\".0.3.0\"",
        "\".0.4\"",
    ];
    for child_path_str in CHILD_PATH_STRS {
        let child = unwrap_child_path(serde_json::from_str(child_path_str)?);
        let child_ref = child.try_ref(&mut t).expect("child exists");
        assert_eq!(child_ref.filter, None);
    }
    //
    assert_eq!(t.pop_item_queued(), None);
    {
        // un-block node "0"
        let base_path = unwrap_child_path(serde_json::from_str("\".0\"")?);
        let mut base_ref = base_path.try_ref(&mut t).expect("base exists");
        base_ref.set_weight(1);
    }
    assert_eq!(t.pop_item_queued(), None);
    {
        // un-block node "0.4"
        let child4_path = unwrap_child_path(serde_json::from_str("\".0.4\"")?);
        let mut child4_ref = child4_path.try_ref(&mut t).expect("child4 exists");
        child4_ref.set_weight(1);
    }
    for _ in 0..100 {
        assert_eq!(t.pop_item(), Some("ping"));
        assert_eq!(t.pop_item(), Some("pong"));
    }

    // remove ping/pong node
    assert_eq!(t.sum_node_count(), 8);
    assert_eq!(t.pop_item(), Some("ping"));
    assert_eq!(t.pop_item(), Some("pong"));
    {
        let child4_id = unwrap_child_id(serde_json::from_str("\".0.4#7\"")?);
        let (weight, node_info) = t
            .remove_node(&child4_id)
            .expect("child4 exists")
            .expect("child4 remove succeeds");
        assert_eq!(weight, 1);
        match node_info {
            NodeInfo::Items { items, .. } => assert_eq!(items, vec!["ping", "pong"]),
            other => panic!("unexpected node_info from removed: {other:?}"),
        }
    }
    assert_eq!(t.sum_node_count(), 7);
    assert_eq!(t.pop_item(), None);
    assert_eq!(t.pop_item(), None);

    Ok(())
}
