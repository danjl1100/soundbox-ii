// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use self::{
    generic_order_opt::{OrderNodeImpl as _, OrderNodeSlice, OrderNodeSliceImpl},
    simple_visitor::SimpleVisitor,
};
use crate::{
    child_vec::{ChildVec, Weights},
    order,
    path::{Path, PathRef},
    Bucket, Child, Joint, Trees, UnknownPathRef,
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

mod generic_order_opt {
    use crate::order;

    pub(crate) trait OrderNodeSliceImpl {
        type Node: OrderNodeImpl<Self>;
        fn get(&self, index: usize) -> Option<&Self::Node>;
        fn assert_len(&self, expected_len: usize, message: &str);
        fn assert_bucket_empty(&self);
    }
    pub(crate) trait OrderNodeImpl<S: ?Sized> {
        fn get_children(&self) -> &S;
    }

    pub(super) type OrderNodeSlice = [std::rc::Rc<order::OrderNode>];
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
}

impl<T, U> Trees<T, U> {
    #[cfg(test)]
    pub(crate) fn assert_topologies_match(&self) {
        // traversing the tree checks the topologies match
        self.visit_depth_first(|_| {});
    }

    fn subtree_root(&self) -> Subtrees<'_, OrderNodeSlice, T, U> {
        Subtrees {
            path: Path::empty(),
            child_items: &self.item,
            child_orders: self.order.node().get_children(),
            child_start_index: 0,
        }
    }
    #[cfg(test)]
    fn subtree_root_items(&self) -> Subtrees<'_, (), T, U> {
        Subtrees {
            path: Path::empty(),
            child_items: &self.item,
            child_orders: &(),
            child_start_index: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn visit_depth_first_items(
        &self,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, (), T, U>),
    ) {
        self.subtree_root_items().visit_depth_first(visit_fn);
    }
    pub(crate) fn visit_depth_first(
        &self,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, order::OrderNode, T, U>),
    ) {
        self.subtree_root().visit_depth_first(visit_fn);
    }
    pub(crate) fn try_visit_depth_first<E>(
        &self,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, order::OrderNode, T, U>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.subtree_root().try_visit_depth_first(visit_fn)
    }
    pub(crate) fn visit_depth_first_items_at(
        path: Path,
        child_items: &ChildVec<Child<T, U>>,
        visit_fn: impl for<'a> FnMut(TraversalElem<'a, (), T, U>),
    ) {
        Subtrees {
            path,
            child_items,
            child_orders: &(),
            child_start_index: 0,
        }
        .visit_depth_first(visit_fn);
    }
}

pub(crate) trait DepthFirstVisitor<T, U, E, S: ?Sized + OrderNodeSliceImpl = OrderNodeSlice> {
    fn visit(&mut self, elem: TraversalElem<'_, S::Node, T, U>) -> Result<(), E>;
    fn finalize_after_children(&mut self, _path: PathRef<'_>, child_sum: usize) -> usize {
        child_sum
    }
}

mod simple_visitor {
    use super::{generic_order_opt::OrderNodeSliceImpl, DepthFirstVisitor, TraversalElem};

    pub(super) enum Never {}
    pub(super) struct SimpleVisitor<F>(pub F);
    impl<S: ?Sized, T, U, F> DepthFirstVisitor<T, U, Never, S> for SimpleVisitor<F>
    where
        S: OrderNodeSliceImpl,
        F: for<'b> FnMut(TraversalElem<'b, S::Node, T, U>),
    {
        fn visit(&mut self, elem: TraversalElem<'_, S::Node, T, U>) -> Result<(), Never> {
            (self.0)(elem);
            Ok(())
        }
    }
}

impl<S: ?Sized, T, U, E, F> DepthFirstVisitor<T, U, E, S> for F
where
    S: OrderNodeSliceImpl,
    F: for<'a> FnMut(TraversalElem<'a, S::Node, T, U>) -> Result<(), E>,
{
    fn visit(&mut self, elem: TraversalElem<'_, S::Node, T, U>) -> Result<(), E> {
        self(elem)
    }
}

pub(crate) struct Subtrees<'a, S: ?Sized, T, U> {
    path: Path,
    child_items: &'a ChildVec<Child<T, U>>,
    child_orders: &'a S,
    child_start_index: usize,
}
impl<S: ?Sized, T, U> std::fmt::Debug for Subtrees<'_, S, T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            path,
            child_items: _,  // complicated bounds
            child_orders: _, // complicated bounds
            child_start_index,
        } = self;
        f.debug_struct("Subtrees")
            .field("path", path)
            .field("child_start_index", child_start_index)
            .finish()
    }
}
impl<'a, S: ?Sized, T, U> Subtrees<'a, S, T, U> {
    fn visit_depth_first<F>(&mut self, visit_fn: F)
    where
        S: OrderNodeSliceImpl,
        F: for<'b> FnMut(TraversalElem<'b, S::Node, T, U>),
    {
        self.try_visit_depth_first(SimpleVisitor(visit_fn))
            .unwrap_or_else(|n| match n {});
    }
    pub(crate) fn try_visit_depth_first<E>(
        &mut self,
        mut visitor: impl DepthFirstVisitor<T, U, E, S>,
    ) -> Result<(), E>
    where
        S: OrderNodeSliceImpl,
    {
        let Self {
            ref mut path,
            child_items,
            child_orders,
            child_start_index,
        } = *self;
        let mut stack = vec![(child_start_index, child_items, child_orders, 0)];
        while let Some(last) = stack.last() {
            let (index, child_items, child_order, _prev_sum) = *last;
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
                let (_, _, _, sum) = stack
                    .pop()
                    .expect("stack should not double pop when last existed");
                visitor.finalize_after_children(path.as_ref(), sum);
                path.pop();
                continue;
            };

            path.push(index);

            visitor.visit(TraversalElem {
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
                        Some((0, &joint.next, order_children, 0))
                    }
                }
            };

            let last = stack
                .last_mut()
                .expect("last should be available within the loop");
            last.0 += 1;
            last.3 += 1;

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

impl<T, U> ChildVec<Child<T, U>> {
    /// Returns the children at the path (if any) and the matched node (if not root)
    pub(crate) fn for_each_direct_child<'a, 'b>(
        &'a self,
        path: PathRef<'b>,
        mut process_child_fn: impl FnMut(&'a Child<T, U>),
    ) -> Result<OptChildrenAndChildRef<'a, T, U>, UnknownPathRef<'b>> {
        let mut current = Some(self);
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
        &'a mut self,
        bucket_path: PathRef<'b>,
    ) -> Result<Option<&'a mut Bucket<T, U>>, UnknownPathRef<'b>> {
        match self.find_child_mut(bucket_path)? {
            ChildFound::Bucket(bucket) => Ok(Some(bucket)),
            _ => Ok(None),
        }
    }

    // TODO deleteme, most shared-ref searching is for "Item and Order" case
    // fn find_child<'a, 'b>(
    //     &'a self,
    //     path: PathRef<'b>,
    // ) -> Result<ChildFound<'a, T, U>, UnknownPathRef<'b>> {
    //     let mut current = ChildFound::RootChildren(self);

    //     for next_index in path {
    //         let Some(next_child) = current
    //             .into_child_vec()
    //             .and_then(|c| c.children().get(next_index))
    //         else {
    //             return Err(UnknownPathRef(path));
    //         };
    //         current = match next_child {
    //             Child::Bucket(bucket) => ChildFound::Bucket(bucket),
    //             Child::Joint(joint) => ChildFound::Joint(joint),
    //         };
    //     }
    //     Ok(current)
    // }

    pub(crate) fn find_child_mut<'a, 'b>(
        &'a mut self,
        path: PathRef<'b>,
    ) -> Result<ChildFound<'a, T, U>, UnknownPathRef<'b>> {
        let mut current = ChildFound::RootChildren(self);

        for next_index in path {
            let Some(next_child) = current
                .into_child_vec()
                .and_then(|c| c.children_mut().get_mut(next_index))
            else {
                return Err(UnknownPathRef(path));
            };
            current = match next_child {
                Child::Bucket(bucket) => ChildFound::Bucket(bucket),
                Child::Joint(joint) => ChildFound::Joint(joint),
            };
        }
        Ok(current)
    }
}

pub(crate) enum ChildFound<'a, T, U> {
    /// Root is not a node, so only represent the [`ChildVec`]
    RootChildren(&'a mut ChildVec<Child<T, U>>),
    /// Joint node
    Joint(&'a mut Joint<T, U>),
    /// Bucket node
    Bucket(&'a mut Bucket<T, U>),
}
impl<'a, T, U> ChildFound<'a, T, U> {
    fn into_child_vec(self) -> Option<&'a mut ChildVec<Child<T, U>>> {
        match self {
            Self::RootChildren(children) => Some(children),
            Self::Joint(joint) => Some(&mut joint.next),
            Self::Bucket(_) => None,
        }
    }
}
