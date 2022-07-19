// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::{borrow::Cow, collections::VecDeque};

use crate::{DebugItemSource, Error, ItemSource, Sequencer};
use q_filter_tree::OrderType;

mod fail;

#[derive(Default)]
struct UpdateTrackingItemSource(u32);
impl UpdateTrackingItemSource {
    fn set_rev(&mut self, rev: u32) {
        self.0 = rev;
    }
}
impl ItemSource<String> for UpdateTrackingItemSource {
    type Item = String;
    type Error = shared::Never;

    fn lookup(&self, args: &[String]) -> Result<Vec<Self::Item>, Self::Error> {
        let rev = self.0;
        let debug_label = format!("{args:?} rev {rev}");
        Ok((0..10)
            .map(|n| format!("item # {n} for {}", &debug_label))
            .collect())
    }
}

#[test]
fn create_item_node() -> Result<(), Error> {
    let filename = "filename1.txt";

    let mut s = Sequencer::new(DebugItemSource);
    s.add_terminal_node(".", filename.to_string())?;
    for n in 0..10 {
        assert_eq!(
            s.pop_next(),
            Some(Cow::Borrowed(&format!(
                "item # {n} for {:?}",
                vec!["", filename]
            )))
        );
    }

    Ok(())
}

#[test]
fn remove_node() -> Result<(), Error> {
    let mut s = Sequencer::new(DebugItemSource);
    assert_eq!(s.tree.sum_node_count(), 1, "beginning length");
    // add
    s.add_node(".", "".to_string())?;
    assert_eq!(s.tree.sum_node_count(), 2, "length after add");
    // remove
    let expect_removed = q_filter_tree::NodeInfo::Chain {
        queue: VecDeque::new(),
        filter: String::new(),
        order: OrderType::default(),
    };
    assert_eq!(s.remove_node(".0#1")?, (1, expect_removed));
    assert_eq!(s.tree.sum_node_count(), 1, "length after removal");
    Ok(())
}

fn assert_next(
    sequencer: &mut Sequencer<UpdateTrackingItemSource, String>,
    filters: &[&str],
    sequence: usize,
    rev: usize,
) {
    assert_eq!(
        sequencer.pop_next(),
        Some(Cow::Borrowed(&format!(
            "item # {sequence} for {filters:?} rev {rev}"
        )))
    );
}
#[test]
fn update_node() -> Result<(), Error> {
    let filename = "foo_bar_file";

    let mut s = Sequencer::new(UpdateTrackingItemSource(0));
    s.add_terminal_node(".", filename.to_string())?;
    let filters = vec!["", filename];
    assert_next(&mut s, &filters, 0, 0);
    assert_next(&mut s, &filters, 1, 0);
    assert_next(&mut s, &filters, 2, 0);
    //
    s.ref_item_source().set_rev(52);
    assert_next(&mut s, &filters, 3, 0);
    assert_next(&mut s, &filters, 4, 0);
    assert_next(&mut s, &filters, 5, 0);
    s.update_node(".")?;
    assert_next(&mut s, &filters, 6, 52);
    assert_next(&mut s, &filters, 7, 52);
    assert_next(&mut s, &filters, 8, 52);
    Ok(())
}
#[test]
fn update_subtree() -> Result<(), Error> {
    let mut s = Sequencer::new(UpdateTrackingItemSource(0));
    s.add_node(".", "base1".to_string())?;
    s.add_terminal_node(".0", "child1".to_string())?;
    s.add_terminal_node(".0", "child2".to_string())?;
    s.add_node(".", "base2".to_string())?;
    s.add_terminal_node(".1", "child3".to_string())?;
    let filters_child1 = vec!["", "base1", "child1"];
    let filters_child2 = vec!["", "base1", "child2"];
    let filters_child3 = vec!["", "base2", "child3"];
    //
    assert_next(&mut s, &filters_child1, 0, 0);
    assert_next(&mut s, &filters_child3, 0, 0);
    assert_next(&mut s, &filters_child2, 0, 0);
    assert_next(&mut s, &filters_child3, 1, 0);
    //
    s.ref_item_source().set_rev(5);
    assert_next(&mut s, &filters_child1, 1, 0);
    assert_next(&mut s, &filters_child3, 2, 0);
    assert_next(&mut s, &filters_child2, 1, 0);
    assert_next(&mut s, &filters_child3, 3, 0);
    s.update_node(".1.0")?;
    assert_next(&mut s, &filters_child1, 2, 0);
    assert_next(&mut s, &filters_child3, 4, 5);
    assert_next(&mut s, &filters_child2, 2, 0);
    assert_next(&mut s, &filters_child3, 5, 5);
    //
    s.ref_item_source().set_rev(8);
    assert_next(&mut s, &filters_child1, 3, 0);
    assert_next(&mut s, &filters_child3, 6, 5);
    assert_next(&mut s, &filters_child2, 3, 0);
    assert_next(&mut s, &filters_child3, 7, 5);
    s.update_node(".1")?;
    assert_next(&mut s, &filters_child1, 4, 0);
    assert_next(&mut s, &filters_child3, 8, 8);
    assert_next(&mut s, &filters_child2, 4, 0);
    assert_next(&mut s, &filters_child3, 9, 8);
    //
    s.ref_item_source().set_rev(9);
    assert_next(&mut s, &filters_child1, 5, 0);
    assert_next(&mut s, &filters_child3, 0, 8);
    assert_next(&mut s, &filters_child2, 5, 0);
    assert_next(&mut s, &filters_child3, 1, 8);
    s.update_node(".0")?;
    assert_next(&mut s, &filters_child1, 6, 9);
    assert_next(&mut s, &filters_child3, 2, 8);
    assert_next(&mut s, &filters_child2, 6, 9);
    assert_next(&mut s, &filters_child3, 3, 8);
    Ok(())
}
