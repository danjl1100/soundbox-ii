use super::Weight;

pub enum State {
    Empty(Type),
    State(Box<dyn Order>),
}
impl State {
    /// Returns the [`Type`] of the State
    pub(crate) fn get_type(&self) -> Type {
        match self {
            Self::Empty(ty) => *ty,
            Self::State(order) => order.get_type(),
        }
    }
    /// Clears the state, leaving only the [`Type`]
    pub(crate) fn clear(&mut self) {
        *self = Self::Empty(self.get_type());
    }
    /// Retrieves the next index from the [`Order`], instantiating if necessary
    pub(crate) fn next(&mut self, weights: &[Weight]) -> Option<usize> {
        self.get_state(weights).next(weights)
    }
    /// Instantiates the state (if needed) to the specified weights
    fn get_state(&mut self, weights: &[Weight]) -> &mut Box<dyn Order> {
        match self {
            Self::State(state) => state,
            Self::Empty(ty) => {
                *self = Self::State(ty.instantiate(weights));
                match self {
                    Self::State(state) => state,
                    Self::Empty(_) => unreachable!(),
                }
            }
        }
    }
}
impl From<Type> for State {
    fn from(ty: Type) -> Self {
        Self::Empty(ty)
    }
}
impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        self.get_type() == other.get_type()
    }
}
impl Eq for State {}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let variant = match self {
            Self::Empty(_) => "Empty",
            Self::State(_) => "State",
        };
        let ty = self.get_type();
        write!(f, "State::{}({:?})", variant, ty)
    }
}

/// Order of picking nodes from children nodes, given the node [`Weight`]s.
///
/// # Examples:
///
/// 1. [`Self::InOrder`]
///
/// Visits child nodes **in order**.  Weights `[2, 1, 3]` will yield `AABCCC AABCCC ...`
/// ```
/// use q_filter_tree::{Tree, PopError, OrderType};
/// let mut t: Tree<_, ()> = Tree::default();
/// let root = t.root_id();
/// //
/// t.set_order(&root, OrderType::InOrder);
/// //
/// let childA = t.add_child(&root, Some(2)).unwrap();
/// t.push_item(&childA, "A1").unwrap();
/// t.push_item(&childA, "A2").unwrap();
/// let childB = t.add_child(&root, Some(1)).unwrap();
/// t.push_item(&childB, "B1").unwrap();
/// let childC = t.add_child(&root, Some(3)).unwrap();
/// t.push_item(&childC, "C1").unwrap();
/// t.push_item(&childC, "C2").unwrap();
/// t.push_item(&childC, "C3").unwrap();
/// //
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A1"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A2"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("B1"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C1"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C2"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C3"));
/// assert_eq!(t.pop_item_from(&root).unwrap(), Err(PopError::Empty(root)));
/// ```
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Type {
    /// Picks [`Weight`] items from one node before moving to the next node
    InOrder,
}
impl Type {
    /// Creates an instance of the specified `Order` type
    pub(crate) fn instantiate(self, weights: &[Weight]) -> Box<dyn Order> {
        match self {
            Type::InOrder => Box::new(InOrderState::new(weights)),
        }
    }
}

pub trait Order {
    fn get_type(&self) -> Type;
    fn resize_to(&mut self, weights: &[Weight]);
    fn get_weights(&self) -> &[Weight];
    fn next_unchecked(&mut self) -> Option<usize>;
    fn next(&mut self, weights: &[Weight]) -> Option<usize> {
        if self.get_weights() != weights {
            self.resize_to(weights);
        }
        self.next_unchecked()
    }
}

struct InOrderState {
    weights: Vec<Weight>,
    index_remaining: Option<(usize, Weight)>,
}
impl InOrderState {
    fn new(weights: &[Weight]) -> Self {
        let mut this = Self {
            weights: vec![],
            index_remaining: None,
        };
        this.resize_to(weights);
        this
    }
}
impl Order for InOrderState {
    fn get_type(&self) -> Type {
        Type::InOrder
    }
    fn resize_to(&mut self, weights: &[Weight]) {
        self.weights = weights.to_vec();
        self.index_remaining = None;
    }
    fn get_weights(&self) -> &[Weight] {
        &self.weights
    }
    fn next_unchecked(&mut self) -> Option<usize> {
        let filter_nonzero_weight = |(index, &weight)| {
            if weight > 0 {
                Some((index, weight - 1))
            } else {
                None
            }
        };
        self.index_remaining = self
            .index_remaining
            .and_then(|(index, weight)| {
                if weight > 0 {
                    Some((index, weight - 1))
                } else {
                    let index = index + 1;
                    // search Tail then Head for first non-zero weight
                    let tail = self.weights.iter().enumerate().skip(index);
                    let head = self.weights.iter().enumerate();
                    tail.chain(head).find_map(filter_nonzero_weight)
                }
            })
            .or_else(|| {
                // find first index of non-zero weight
                self.weights
                    .iter()
                    .enumerate()
                    .find_map(filter_nonzero_weight)
            });
        // next index
        self.index_remaining.map(|(index, _)| index)
    }
}
#[cfg(test)]
mod tests {
    use super::Type;
    fn check_simple(ty: Type) {
        let weights = &[1];
        let mut s = ty.instantiate(weights);
        for _ in 0..100 {
            assert_eq!(s.next(weights), Some(0));
        }
    }
    fn check_blocked(ty: Type) {
        let weights = &[0];
        let mut s = ty.instantiate(weights);
        for _ in 0..100 {
            assert_eq!(s.next(weights), None);
        }
    }
    // Type::InOrder
    #[test]
    fn in_order_simple() {
        check_simple(Type::InOrder);
    }
    #[test]
    fn in_order_blocked() {
        check_blocked(Type::InOrder);
    }
    #[test]
    fn in_order_longer() {
        let weights = &[1, 2, 2, 3, 0, 5];
        let mut s = Type::InOrder.instantiate(weights);
        for _ in 0..100 {
            for (index, &weight) in weights.iter().enumerate() {
                for _ in 0..weight {
                    assert_eq!(s.next(weights), Some(index));
                    //
                    // let value = s.next(weights);
                    // let expected = Some(index);
                    // assert_eq!(value, expected);
                    // println!("{:?} = {:?} ??", value, expected);
                }
            }
        }
    }
}
