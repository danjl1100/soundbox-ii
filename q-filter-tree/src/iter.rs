use crate::{id::NodeId, node::Node, Tree};

impl<T, F> Tree<T, F> {
    /// Creates a depth-first iterator over [`NodeId`]s
    pub fn iter_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.enumerate().map(|(id, _)| id)
    }
    /// Creates a depth-first iterator over [`NodeId`] and Nodes
    pub fn enumerate(&self) -> impl Iterator<Item = (NodeId, &'_ Node<T, F>)> + '_ {
        Iter {
            tree: self,
            next_id: Some(self.root_id()),
        }
    }
}
struct Iter<'a, T, F> {
    tree: &'a Tree<T, F>,
    next_id: Option<NodeId>,
}
impl<'a, T, F> Iterator for Iter<'a, T, F> {
    type Item = (NodeId, &'a Node<T, F>);
    fn next(&mut self) -> Option<Self::Item> {
        self.next_id.take().and_then(|cur_id| {
            self.tree
                .get_node_and_next_id(&cur_id)
                .map(|(cur_node, next_id)| {
                    self.next_id = next_id;
                    (cur_id, cur_node)
                })
                .ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Tree;
    #[test]
    fn empty() {
        let t: Tree<(), ()> = Tree::default();
        let root = t.root_id();
        //
        let mut iter = t.iter_ids();
        assert_eq!(iter.next(), Some(root));
        for _ in 0..20 {
            assert_eq!(iter.next(), None);
        }
    }
    #[test]
    fn single() {
        let mut t: Tree<(), ()> = Tree::default();
        let root = t.root_id();
        //
        let child1 = t.add_child(&root, None).expect("root exists");
        //
        let mut iter = t.iter_ids();
        assert_eq!(iter.next(), Some(root));
        assert_eq!(iter.next(), Some(child1));
        for _ in 0..20 {
            assert_eq!(iter.next(), None);
        }
    }
    #[test]
    fn complex() {
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
        let child1 = t.add_child(&base, None).expect("base exists");
        let child2 = t.add_child(&base, None).expect("base exists");
        let child3 = t.add_child(&base, None).expect("base exists");
        let child4 = t.add_child(&base, None).expect("base exists");
        let child5 = t.add_child(&base, None).expect("base exists");
        let child4_child = t.add_child(&child4, None).expect("child4 exists");
        //
        let mut iter = t.iter_ids();
        assert_eq!(iter.next(), Some(root));
        assert_eq!(iter.next(), Some(base));
        assert_eq!(iter.next(), Some(child1));
        assert_eq!(iter.next(), Some(child2));
        assert_eq!(iter.next(), Some(child3));
        assert_eq!(iter.next(), Some(child4));
        assert_eq!(iter.next(), Some(child4_child));
        assert_eq!(iter.next(), Some(child5));
        for _ in 0..20 {
            assert_eq!(iter.next(), None);
        }
    }
}
