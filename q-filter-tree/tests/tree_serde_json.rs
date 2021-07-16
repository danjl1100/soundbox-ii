use q_filter_tree::Tree;
use serde_json::Result;

#[test]
#[ignore]
fn simple() -> Result<()> {
    let mut t: Tree<(), ()> = Tree::new();
    let root = t.root_id();
    //
    let _child = t.add_child(&root, None).expect("root exists");
    //
    let json_str = serde_json::to_string(&t)?;
    assert_eq!(json_str, r#"idk, some string!"#);
    Ok(())
}
