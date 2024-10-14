// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    child_vec::{ChildVec, Weights},
    order,
    path::{Path, PathRef},
    Bucket, Child, Network, UnknownPathRef,
};

#[derive(Clone, Copy)]
pub(crate) struct TraversalElem<'a, O, T, U> {
    pub node_path: PathRef<'a>,
    /// Weight entries in the parent, or `None` if no weights exist (all zero)
    pub parent_weights: Option<Weights<'a>>,
    pub node_weight: u32,
    pub node_item: &'a Child<T, U>,
    pub node_order: &'a O,
}

trait OrderNodeSliceImpl {
    type Node: OrderNodeImpl<Self>;
    fn get(&self, index: usize) -> Option<&Self::Node>;
    fn assert_len(&self, expected_len: usize, message: &str);
    fn assert_bucket_empty(&self);
}
trait OrderNodeImpl<S: ?Sized> {
    fn get_children(&self) -> &S;
}

type OrderNodeSlice = [std::rc::Rc<order::OrderNode>];
impl OrderNodeSliceImpl for OrderNodeSlice {
    type Node = order::OrderNode;
    fn get(&self, index: usize) -> Option<&Self::Node> {
        self.get(index).map(|n| &**n)
    }
    fn assert_len(&self, expected_len: usize, message: &str) {
        assert_eq!(self.len(), expected_len, "{message}");
    }
    fn assert_bucket_empty(&self) {
        debug_assert!(self.is_empty(), "bucket order-children should be empty");
    }
}
impl OrderNodeImpl<OrderNodeSlice> for order::OrderNode {
    fn get_children(&self) -> &OrderNodeSlice {
        self.get_children()
    }
}

impl OrderNodeSliceImpl for () {
    type Node = ();
    fn get(&self, _: usize) -> Option<&Self::Node> {
        // allow "no-op traversal"
        Some(self)
    }
    fn assert_len(&self, _: usize, _: &str) {} // empty
    fn assert_bucket_empty(&self) {} // empty
}
impl OrderNodeImpl<()> for () {
    fn get_children(&self) -> &() {
        // allow "no-op traversal"
        self
    }
}

impl<T, U> Network<T, U> {
    #[cfg(test)]
    pub(crate) fn assert_tree_topologies_match(&self) {
        enum Never {}

        // traversing the tree checks the topologies match

        Self::depth_first_traversal(
            &mut Path::empty(),
            &self.root,
            self.root_order.node().get_children(),
            |_| Ok(()),
        )
        .unwrap_or_else(|n: Never| match n {});
    }
    pub(crate) fn depth_first_traversal_items<E>(
        path: &mut Path,
        child_items: &ChildVec<Child<T, U>>,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, (), T, U>) -> Result<(), E>,
    ) -> Result<(), E> {
        Self::depth_first_traversal(path, child_items, &(), visit_fn)
    }
    // TODO when Network refactored into sub-field Tree, we can borrow entire `&mut self` and iterate from root
    // pub(crate) fn depth_first_traversal_items_order_root<E>(
    //     &self,
    //     visit_fn: impl for<'a> FnMut(TraversalElem<'a, order::OrderNode, T, U>) -> Result<(), E>,
    // ) -> Result<(), E> {
    //     let path = &mut Path::empty();
    //     let child_items = &self.root;
    //     let child_order = self.root_order.node().get_children();
    //     Self::depth_first_traversal(path, child_items, child_order, visit_fn)
    // }
    pub(crate) fn depth_first_traversal_items_order<E>(
        path: &mut Path,
        child_items: &ChildVec<Child<T, U>>,
        child_order: &OrderNodeSlice,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, order::OrderNode, T, U>) -> Result<(), E>,
    ) -> Result<(), E> {
        Self::depth_first_traversal(path, child_items, child_order, visit_fn)
    }
    fn depth_first_traversal<E, S>(
        path: &mut Path,
        child_items: &ChildVec<Child<T, U>>,
        child_order: &S,
        mut visit_fn: impl for<'a> FnMut(TraversalElem<'a, S::Node, T, U>) -> Result<(), E>,
    ) -> Result<(), E>
    where
        S: OrderNodeSliceImpl + ?Sized,
    {
        let mut stack = vec![(0, child_items, child_order)];
        while let Some(last) = stack.last() {
            let (index, child_items, child_order) = *last;
            child_order.assert_len(
                child_items.len(),
                "items children length should match order children length",
            );

            let node_item = child_items.children().get(index);
            let node_order = child_order.get(index);

            let child_weights = child_items.weights();
            let node_weight = child_weights.map_or(Some(0), |w| w.get(index));

            let Some(((node_item, node_order), node_weight)) =
                node_item.zip(node_order).zip(node_weight)
            else {
                path.pop();
                stack.pop();
                continue;
            };

            path.push(index);

            visit_fn(TraversalElem {
                node_path: path.as_ref(),
                parent_weights: child_weights,
                node_weight,
                node_item,
                node_order,
            })?;

            let order_children = node_order.get_children();

            let inner_child = match node_item {
                Child::Bucket(_) => {
                    order_children.assert_bucket_empty();
                    None
                }
                Child::Joint(joint) => {
                    if joint.next.is_empty() {
                        None
                    } else {
                        Some((0, &joint.next, order_children))
                    }
                }
            };

            let last = stack
                .last_mut()
                .expect("last should be available within the loop");
            last.0 += 1;

            if let Some(inner_child) = inner_child {
                stack.push(inner_child);
            } else {
                path.pop();
            }
        }

        Ok(())
    }
}

type OptChildrenRef<'a, T, U> = Option<&'a ChildVec<Child<T, U>>>;
type OptChildRef<'a, T, U> = Option<&'a Child<T, U>>;
type OptChildrenAndChildRef<'a, T, U> = (OptChildrenRef<'a, T, U>, OptChildRef<'a, T, U>);

impl<T, U> Network<T, U> {
    /// Returns the children at the path (if any) and the matched node (if not root)
    pub(crate) fn for_each_child<'a, 'b>(
        &'a self,
        path: PathRef<'b>,
        mut process_child_fn: impl FnMut(&'a Child<T, U>),
    ) -> Result<OptChildrenAndChildRef<'a, T, U>, UnknownPathRef<'b>> {
        let mut current = Some(&self.root);
        let mut found = None;

        for next_index in path {
            let Some(next_child) = current.and_then(|c| c.children().get(next_index)) else {
                return Err(UnknownPathRef(path));
            };

            process_child_fn(next_child);
            found = Some(next_child);

            current = match next_child {
                Child::Bucket(_) => None,
                Child::Joint(joint) => Some(&joint.next),
            };
        }

        Ok((current, found))
    }

    pub(crate) fn find_bucket_mut<'a, 'b>(
        root_items: &'a mut ChildVec<Child<T, U>>,
        bucket_path: PathRef<'b>,
    ) -> Result<Option<&'a mut Bucket<T, U>>, UnknownPathRef<'b>> {
        let mut current = root_items;

        let mut bucket_path_iter = bucket_path.into_iter();
        let dest_bucket = loop {
            let Some(next_index) = bucket_path_iter.next() else {
                return Ok(None); // joint
            };
            let Some(next_child) = current.children_mut().get_mut(next_index) else {
                return Err(UnknownPathRef(bucket_path));
            };
            current = match next_child {
                Child::Bucket(bucket) => break bucket,
                Child::Joint(joint) => &mut joint.next,
            };
        };
        if bucket_path_iter.next().is_some() {
            return Err(UnknownPathRef(bucket_path));
        }
        Ok(Some(dest_bucket))
    }
}
