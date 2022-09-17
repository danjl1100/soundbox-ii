// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{sources::ItemSource, Error, Sequencer};

struct FailSource {
    allowed_count: usize,
}
impl ItemSource<()> for FailSource {
    type Item = shared::Never;
    type Error = &'static str;

    fn lookup(&self, args: &[()]) -> Result<Vec<Self::Item>, Self::Error> {
        (args.len() == self.allowed_count)
            .then_some(vec![])
            .ok_or("fail")
    }
}

fn build_dummy_path(length: usize) -> String {
    if length == 0 {
        ".".to_string()
    } else {
        ".0".repeat(length)
    }
}

#[test]
#[allow(clippy::panic)]
fn lookup_fails() {
    let allowed_count = 3;
    let mut s = Sequencer::new(FailSource { allowed_count }, ());
    for count in 0..100 {
        let parent_path = build_dummy_path(count);
        let child_count = count + 1;
        let nodes_count = count + 2;
        let child_path = build_dummy_path(child_count);
        let result = s.add_terminal_node(dbg!(&parent_path), ());
        if nodes_count == allowed_count {
            assert!(result.is_ok(), "unexpected err {result:?}");
        } else {
            assert!(
                matches!(result, Err(Error::Message(ref message)) if message == "item lookup error: fail"),
                "unexpected result {result:?}"
            );
        }
        let sequence = 2 * count + 1;
        let _removed = s
            .remove_node(&format!("{child_path}#{sequence}"))
            .expect("remove node");
        s.add_node(&parent_path, ()).expect("re-add node");
    }
}
