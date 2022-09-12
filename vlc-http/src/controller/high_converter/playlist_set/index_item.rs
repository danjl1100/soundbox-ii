// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Identifies where the desired current item is located in the playlist

use super::{Command, PlaylistItem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IndexItem<'a> {
    /// Type of indexed item, some DesiredType, or none for undesired current item
    pub ty: Option<DesiredType>,
    /// Index of the item
    pub index: usize,
    /// Reference to the item
    pub item: &'a PlaylistItem,
}
pub(super) type Type = Option<DesiredType>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DesiredType {
    /// Item is the commanded item, and it is current
    Current,
    /// Item is the commanded item, and it is immediately prior to the current
    Previous,
}
impl<'a> IndexItem<'a> {
    pub fn new(
        current_id: Option<u64>,
        items: &'a [PlaylistItem],
        command: &Command,
    ) -> Option<Self> {
        // find current index
        let current_index = {
            let current_id_str = current_id?.to_string();
            items
                .iter()
                .enumerate()
                .find_map(|(index, item)| (item.id == current_id_str).then_some(index))?
        };
        let command_url_str = command.urls.first()?.to_string();
        //NOTE panics if caller doesn't provide index matching the provided slice
        let current_item = items
            .get(current_index)
            .expect("current_index with bounds of provided items");
        let current = {
            // check current item
            (current_item.url == command_url_str).then_some(IndexItem {
                ty: Some(DesiredType::Current),
                index: current_index,
                item: current_item,
            })
        };
        let previous = || {
            // check previous item
            let previous_index = current_index.checked_sub(1)?;
            let previous_item = items
                .get(previous_index)
                .expect("current_index - 1 within bounds, since current_index was");
            (previous_item.url == command_url_str).then_some(IndexItem {
                ty: Some(DesiredType::Previous),
                index: previous_index,
                item: previous_item,
            })
        };
        // no match found to "desired" (command)
        let undesired = IndexItem {
            ty: None,
            index: current_index,
            item: current_item,
        };
        Some(current.or_else(previous).unwrap_or(undesired))
    }
    pub fn get_comparison_start(&self) -> usize {
        match self.ty {
            Some(_) => self.index,  // include desired in comparison
            None => self.index + 1, // keep undesired, compare after
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::file_url;
    use super::*;

    fn cmd(current: &str) -> Command {
        Command {
            urls: vec![file_url(current)],
            max_history_count: 1.try_into().expect("nonzero"),
        }
    }
    #[test]
    fn empty() {
        assert_eq!(IndexItem::new(None, &[], &cmd("desired")), None);
    }
    #[test]
    fn single() {
        let cmd = &cmd("desired");
        let items = &items![22=>"desired"];
        assert_eq!(IndexItem::new(None, items, cmd), None);
        assert_eq!(IndexItem::new(Some(21), items, cmd), None);
        assert_eq!(
            IndexItem::new(Some(22), items, cmd),
            Some(IndexItem {
                ty: Some(DesiredType::Current),
                index: 0,
                item: &items[0],
            })
        );
    }
    #[test]
    fn double() {
        let items = &items![54=>"first", 42=>"second"];
        let item = |index, ty| {
            Some(IndexItem {
                ty,
                index,
                item: &items[index],
            })
        };
        {
            let cmd = &cmd("first");
            assert_eq!(IndexItem::new(None, items, cmd), None);
            assert_eq!(IndexItem::new(Some(43), items, cmd), None);
            assert_eq!(
                IndexItem::new(Some(42), items, cmd),
                item(0, Some(DesiredType::Previous)),
            );
            assert_eq!(IndexItem::new(Some(53), items, cmd), None);
            assert_eq!(
                IndexItem::new(Some(54), items, cmd),
                item(0, Some(DesiredType::Current)),
            );
        }
        {
            let cmd = &cmd("second");
            assert_eq!(IndexItem::new(None, items, cmd), None);
            assert_eq!(IndexItem::new(Some(54), items, cmd), item(0, None));
            assert_eq!(IndexItem::new(Some(41), items, cmd), None);
            assert_eq!(
                IndexItem::new(Some(42), items, cmd),
                item(1, Some(DesiredType::Current)),
            );
        }
        {
            let cmd = &cmd("nonexistent");
            assert_eq!(IndexItem::new(None, items, cmd), None);
            assert_eq!(IndexItem::new(Some(54), items, cmd), item(0, None));
            assert_eq!(IndexItem::new(Some(42), items, cmd), item(1, None));
        }
    }
    #[test]
    fn longer() {
        let items = &items![10=>"a", 20=>"b", 30=>"c", 40=>"goto a"];
        let item = |index, ty| {
            Some(IndexItem {
                ty,
                index,
                item: &items[index],
            })
        };
        let previous = |index| item(index, Some(DesiredType::Previous));
        let current = |index| item(index, Some(DesiredType::Current));
        for n in [10, 20, 30, 40] {
            let assert = |commanded, expected| {
                assert_eq!(
                    IndexItem::new(Some(n), items, &cmd(commanded)),
                    expected,
                    "index Some({n}), commanded {commanded:?}"
                );
            };
            let index = (usize::try_from(n).expect("small numbers") / 10) - 1;
            let a = match n {
                10 => current(0),
                20 => previous(0),
                _ => item(index, None),
            };
            let b = match n {
                20 => current(1),
                30 => previous(1),
                _ => item(index, None),
            };
            let c = match n {
                30 => current(2),
                40 => previous(2),
                _ => item(index, None),
            };
            let d = match n {
                40 => current(3),
                _ => item(index, None),
            };
            assert("a", a);
            assert("b", b);
            assert("c", c);
            assert("goto a", d);
            assert("nonexistent", item(index, None));
        }
    }
    #[test]
    fn comparison_start() {
        let items = &items![10=>"a", 20=>"b", 30=>"c", 40=>"goto a"];
        for (index, item) in items.iter().enumerate() {
            let item = |ty| IndexItem { ty, index, item };
            let undesired = item(None);
            let previous = item(Some(DesiredType::Previous));
            let current = item(Some(DesiredType::Current));
            assert_eq!(undesired.get_comparison_start(), index + 1); // keep undesired, compare after
            assert_eq!(previous.get_comparison_start(), index); // need comparison inclusion
            assert_eq!(current.get_comparison_start(), index); // need comparison inclusion
        }
    }
}
