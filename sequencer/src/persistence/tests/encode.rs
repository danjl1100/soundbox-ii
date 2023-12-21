// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    persistence::{IntoKdlEntries, SequencerConfig},
    SequencerTree,
};
use shared::IgnoreNever;

#[derive(Clone, Debug, Default)]
struct FieldsFilter {
    foo: String,
    bar: u32,
    truthiness: bool,
    anonymous_str: String,
}

const TEST_SKIP_ALL_PROPS: &str = "skip_all_props";

struct Error<E>(E);
impl<E> std::fmt::Display for Error<E>
where
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl IntoKdlEntries for FieldsFilter {
    type Error<E> = Error<E> where E: std::fmt::Debug; // E where E: std::fmt::Debug;

    fn try_into_kdl<V: crate::persistence::KdlEntryVisitor>(
        &self,
        mut visitor: V,
    ) -> Result<V, Self::Error<V::Error>> {
        let Self {
            ref foo,
            bar,
            truthiness,
            ref anonymous_str,
        } = *self;

        visitor.visit_argument_str(anonymous_str).map_err(Error)?;

        if anonymous_str != TEST_SKIP_ALL_PROPS {
            visitor.visit_property_str("foo", foo).map_err(Error)?;
            visitor
                .visit_property_i64("bar", bar.into())
                .map_err(Error)?;
            visitor
                .visit_property_bool("truthiness", truthiness)
                .map_err(Error)?;
        }

        Ok(visitor)
    }
}

#[test]
fn updates_root() {
    let old_filter = FieldsFilter {
        foo: "root".to_string(),
        bar: 3,
        truthiness: false,
        anonymous_str: "hiya".to_string(),
    };
    let mut seq_tree = SequencerTree::<(), _>::new(old_filter);

    {
        // mutate tree
        let root_id = seq_tree.tree.root_id();
        let mut tree_guard = seq_tree.guard();
        let mut root = root_id.try_ref(&mut tree_guard.guard);

        let new_filter = FieldsFilter {
            foo: "root".to_string(),
            bar: 3,
            truthiness: false,
            anonymous_str: "hiya".to_string(),
        };

        root.filter = new_filter;
    }

    let mut sequencer_config = SequencerConfig::default();
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    assert_eq!(
        new_doc_str,
        r#"root "hiya" foo="root" bar=3 truthiness=false
"#
        .to_string()
    );
}
#[test]
fn test_remove_attribute() {
    let old_filter = FieldsFilter {
        foo: "value".to_string(),
        bar: 0,
        truthiness: false,
        anonymous_str: "has_props".to_string(),
    };
    let mut seq_tree = SequencerTree::<(), _>::new(old_filter);

    {
        // mutate tree
        let root_id = seq_tree.tree.root_id();
        let mut tree_guard = seq_tree.guard();
        let mut root = root_id.try_ref(&mut tree_guard.guard);

        let new_filter = FieldsFilter {
            foo: "empty".to_string(),
            bar: 0,
            truthiness: false,
            anonymous_str: TEST_SKIP_ALL_PROPS.to_string(),
        };

        root.filter = new_filter;
    }

    let mut sequencer_config = SequencerConfig::default();
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    assert_eq!(
        new_doc_str,
        r#"root "skip_all_props"
"#
        .to_string()
    );
}

#[test]
fn creates_raw_strings() {
    let root_filter = FieldsFilter {
        foo: "quote \" string".to_string(),
        bar: 0,
        truthiness: true,
        anonymous_str: "another \" quoted".to_string(),
    };
    let seq_tree = SequencerTree::<(), _>::new(root_filter);

    let mut sequencer_config = SequencerConfig::default();
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();

    assert_eq!(
        new_doc_str,
        r##"root r#"another " quoted"# foo=r#"quote " string"# bar=0 truthiness=true
"##
    );
}

#[test]
fn keeps_raw_strings_raw() {
    let original_filter = FieldsFilter {
        foo: "starts out \"quoted\"".to_string(),
        bar: 0,
        truthiness: false,
        anonymous_str: "also \"starts\" quoted".to_string(),
    };
    let mut seq_tree = SequencerTree::<(), _>::new(original_filter);

    let mut sequencer_config = SequencerConfig::default();
    let old_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    assert_eq!(
        old_doc_str,
        r##"root r#"also "starts" quoted"# foo=r#"starts out "quoted""# bar=0 truthiness=false
"##
    );

    {
        let root_id = seq_tree.tree.root_id();
        let mut tree_guard = seq_tree.guard();
        let mut root = root_id.try_ref(&mut tree_guard.guard);

        let new_filter = FieldsFilter {
            foo: "no more quotes\nbut newlines".to_string(),
            bar: 0,
            truthiness: false,
            anonymous_str: "here \"quotes\" and \t\t\ttabs".to_string(),
        };

        root.filter = new_filter;
    }

    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    assert_eq!(
        new_doc_str,
        // using non-raw Rust string, for clarity on the tabs (\t) and newlines (\n)
        "root r#\"here \"quotes\" and \t\t\ttabs\"# foo=r\"no more quotes\nbut newlines\" bar=0 truthiness=false
"
    );
}

#[test]
fn add_remove_child() {
    let root_filter = FieldsFilter::default();
    let mut seq_tree = SequencerTree::<(), _>::new(root_filter);

    let root_id = seq_tree.tree.root_id();
    let child1_id;

    // add child with weight
    {
        let mut tree_guard = seq_tree.guard();

        child1_id = tree_guard
            .add_node((&root_id).into(), FieldsFilter::default())
            .expect("root exists");
        tree_guard
            .set_node_weight((&child1_id).into(), 2)
            .expect("child1 exists");
    }

    let mut sequencer_config = SequencerConfig::default();
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    let expected = "root \"\" foo=\"\" bar=0 truthiness=false {
    chain weight=2 \"\" foo=\"\" bar=0 truthiness=false
}
";
    assert_eq!(new_doc_str, expected);

    // alter weight
    {
        let mut tree_guard = seq_tree.guard();

        let mut child1 = child1_id
            .try_ref(&mut tree_guard.guard)
            .expect("child1 exists");
        child1.set_weight(0);
    }
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    let expected = "root \"\" foo=\"\" bar=0 truthiness=false {
    chain weight=0 \"\" foo=\"\" bar=0 truthiness=false
}
";
    assert_eq!(new_doc_str, expected);

    // remove node
    {
        let mut tree_guard = seq_tree.guard();

        let _info = tree_guard.remove_node(&child1_id).expect("child1 removal");
    }
    let new_doc_str = sequencer_config
        .update_to_string(&seq_tree)
        .map_err(|e| e.0)
        .ignore_never();
    // TODO - change back to correct indentation in `expected`
    assert_eq!(
        new_doc_str,
        "root \"\" foo=\"\" bar=0 truthiness=false {
}
"
    );
}
