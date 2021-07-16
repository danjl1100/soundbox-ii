use q_filter_tree::{error::PopError, id::NodePath, Tree};
use serde_json::Result;

const EMPTY_NODE: &'static str =
    r#"{"queue":[],"filter":null,"child_weights":[],"order":"InOrder"}"#;
const ONE_CHILD: &'static str =
    r#"{"queue":[],"filter":null,"child_weights":[0],"order":"InOrder"}"#;
const FIVE_CHILD: &'static str =
    r#"{"queue":[],"filter":null,"child_weights":[0,0,0,0,0],"order":"InOrder"}"#;
#[test]
fn simple_serialize() -> Result<()> {
    let mut t: Tree<(), ()> = Tree::new();
    let root = t.root_id();
    //
    let _child = t.add_child(&root, None).expect("root exists");
    //
    let json_str = serde_json::to_string(&t)?;
    assert_eq!(
        json_str,
        format!(
            r#"{{"":{ONE},"0":{EMPTY}}}"#,
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
    let base = t.add_child(&root, None).expect("root exists");
    let _child1 = t.add_child(&base, None).expect("base exists");
    let _child2 = t.add_child(&base, None).expect("base exists");
    let _child3 = t.add_child(&base, None).expect("base exists");
    let child4 = t.add_child(&base, None).expect("base exists");
    let _child5 = t.add_child(&base, None).expect("base exists");
    let _child4_child = t.add_child(&child4, None).expect("child4 exists");
    //
    let json_str = serde_json::to_string(&t)?;
    assert_eq!(
        json_str,
        format!(
            r#"{{"":{ONE},"0":{FIVE},"0,0":{EMPTY},"0,1":{EMPTY},"0,2":{EMPTY},"0,3":{ONE},"0,3,0":{EMPTY},"0,4":{EMPTY}}}"#,
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
          "": {
            "queue": [],
            "filter": null,
            "child_weights": [
              1
            ],
            "order": "InOrder"
          },
          "0": {
            "queue": [],
            "filter": null,
            "child_weights": [
              0
            ],
            "order": "InOrder"
          },
          "0,0": {
            "queue": ["Alfalfa", "Oats"],
            "filter": null,
            "child_weights": [],
            "order": "InOrder"
          }
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
        t.pop_item_from(&root).expect("root exists"),
        Err(PopError::Blocked((*root).clone()))
    );
    let child: NodePath = serde_json::from_str("\"0,0\"")?;
    t.set_weight(&child, 1).expect("child exists");
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Ok(String::from("Alfalfa"))
    );
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Ok(String::from("Oats"))
    );
    assert_eq!(
        t.pop_item_from(&root).expect("root exists"),
        Err(PopError::Empty(root.into()))
    );
    Ok(())
}

#[test]
#[ignore]
fn complex_deserialize() -> Result<()> {
    let _tree_json = r#"
    {
      "": {
        "queue": [],
        "filter": null,
        "child_weights": [
          0
        ],
        "order": "InOrder"
      },
      "0": {
        "queue": [],
        "filter": null,
        "child_weights": [
          0,
          0,
          0,
          0,
          0
        ],
        "order": "InOrder"
      },
      "0,0": {
        "queue": [],
        "filter": null,
        "child_weights": [],
        "order": "InOrder"
      },
      "0,1": {
        "queue": [],
        "filter": null,
        "child_weights": [],
        "order": "InOrder"
      },
      "0,2": {
        "queue": [],
        "filter": null,
        "child_weights": [],
        "order": "InOrder"
      },
      "0,3": {
        "queue": [],
        "filter": null,
        "child_weights": [
          0
        ],
        "order": "InOrder"
      },
      "0,3,0": {
        "queue": [],
        "filter": null,
        "child_weights": [],
        "order": "InOrder"
      },
      "0,4": {
        "queue": [],
        "filter": null,
        "child_weights": [],
        "order": "InOrder"
      }
    }"#;
    todo!()
}
