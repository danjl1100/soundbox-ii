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

// TODO deleteme, Deserialization is not needed for Serialization tests
// impl FromKdlEntries for FieldsFilter {
//     type Error = ();
//     type Visitor = NoOpVisitor;
//     fn try_finish(visitor: Self::Visitor) -> Result<Self, Self::Error> {
//         // NOTE: ignores all values upon deserialization
//         Ok(Self::default())
//     }
// }
// #[derive(Default)]
// struct NoOpVisitor;
// #[rustfmt::skip]
// impl KdlEntryVisitor for NoOpVisitor {
//     type Error = ();

//     fn visit_property_str(&mut self, _key: &str, _value: &str) -> Result<(), Self::Error> { Ok(()) }
//     fn visit_property_i64(&mut self, _key: &str, _value: i64) -> Result<(), Self::Error> { Ok(()) }
//     fn visit_property_bool(&mut self, _key: &str, _value: bool) -> Result<(), Self::Error> { Ok(()) }

//     fn visit_argument_str(&mut self, _value: &str) -> Result<(), Self::Error> { Ok(()) }
//     fn visit_argument_i64(&mut self, _value: i64) -> Result<(), Self::Error> { Ok(()) }
//     fn visit_argument_bool(&mut self, _value: bool) -> Result<(), Self::Error> { Ok(()) }
// }

impl IntoKdlEntries for FieldsFilter {
    type Error<E> = E;

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

        visitor.visit_argument_str(anonymous_str)?;

        visitor.visit_property_str("foo", foo)?;
        visitor.visit_property_i64("bar", bar.into())?;
        visitor.visit_property_bool("truthiness", truthiness)?;

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
    let mut seq_tree = SequencerTree::<(), FieldsFilter>::new(old_filter);

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
    let new_doc_str = sequencer_config.update_to_string(&seq_tree).ignore_never();
    assert_eq!(
        new_doc_str,
        r#"root "hiya" foo="root" bar=3 truthiness=false
"#
        .to_string()
    );
}
