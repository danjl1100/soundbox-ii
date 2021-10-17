use q_filter_tree::{error::PopError, id::NodePathTyped, Tree};
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
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    let _child = root_ref.add_child(None);
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
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    let base = root_ref.add_child(None);
    let mut base_ref = base.try_ref(&mut t).expect("base exists");
    let _child1 = base_ref.add_child(None);
    let _child2 = base_ref.add_child(None);
    let _child3 = base_ref.add_child(None);
    let child4 = base_ref.add_child(None);
    let _child5 = base_ref.add_child(None);
    let mut child4_ref = child4.try_ref(&mut t).expect("child4 exists");
    let _child4_child = child4_ref.add_child(None);
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
        root.try_ref(&mut t).expect("root exists").pop_item(),
        Err(PopError::Blocked(root.clone().into()))
    );
    let child: NodePathTyped = serde_json::from_str("\"0,0\"")?;
    let child = match child {
        NodePathTyped::Child(path) => path,
        child => panic!("incorrect type on parsed NodePathTyped: {:#?}", child),
    };
    let mut child_ref = child.try_ref(&mut t).expect("child exists");
    child_ref.set_weight(1);
    let mut root_ref = root.try_ref(&mut t).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok(String::from("Alfalfa")));
    assert_eq!(root_ref.pop_item(), Ok(String::from("Oats")));
    assert_eq!(root_ref.pop_item(), Err(PopError::Empty(root.into())));
    Ok(())
}

#[test]
#[ignore] //TODO
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
