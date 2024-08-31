// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

pub(crate) use weights::Weights;

#[derive(Clone, Debug)]
pub(crate) struct ChildVec<T> {
    children: Vec<T>,
    /// Weights for each child (may be empty if all are weighted equally)
    weights: Vec<u32>,
}
impl<T> From<Vec<T>> for ChildVec<T> {
    fn from(children: Vec<T>) -> Self {
        Self {
            children,
            weights: vec![],
        }
    }
}
impl<T> Default for ChildVec<T> {
    fn default() -> Self {
        vec![].into()
    }
}
impl<T> ChildVec<T> {
    pub fn children(&self) -> &[T] {
        &self.children
    }
    /// Returns the non-zero and non-empty weights, or `None` if all zero or empty.
    pub fn weights(&self) -> Option<Weights<'_>> {
        let weights = if self.weights.is_empty() {
            // returns `None` if length is zero
            Weights::new_equal(self.len())?
        } else {
            // returns `None` if weights are all zero
            Weights::new_custom(&self.weights)?
        };
        Some(weights)
    }
    pub fn children_mut(&mut self) -> &mut [T] {
        &mut self.children
    }
    pub fn set_weight(&mut self, index: usize, value: u32) {
        if self.weights.is_empty() {
            self.weights = vec![1; self.len()];
        }
        self.weights[index] = value;
    }
    pub fn len(&self) -> usize {
        self.children.len()
    }
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
    pub fn push(&mut self, child: T) {
        // update to unity weight (if needed)
        if !self.weights.is_empty() {
            self.weights.push(1);
        }

        self.children.push(child);
    }
    pub fn remove(&mut self, index: usize) -> (u32, T) {
        let child = self.children.remove(index);

        let weight = if self.weights.is_empty() {
            1
        } else {
            self.weights.remove(index)
        };
        (weight, child)
    }
}

mod weights {
    /// Non-empty weights (length non-zero, and contents non-zero)
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct Weights<'a>(Inner<'a>);

    #[derive(Clone, Copy, Debug)]
    enum Inner<'a> {
        Unity {
            /// NOTE: specifically chosen to avoid awkward `len = 0` case
            /// e.g. there must be a `next` available for unsigned `max_index >= 0`
            max_index: usize,
        },
        Custom {
            /// Non-empty weights (e.g. at least one nonzero element)
            weights: &'a [u32],
        },
    }
    impl<'a> Weights<'a> {
        /// Returns `None` if the specified slice is empty or all zeros
        pub fn new_custom(weights: &'a [u32]) -> Option<Self> {
            if weights.is_empty() {
                None
            } else {
                assert!(!weights.is_empty());
                weights
                    .iter()
                    .any(|&w| w != 0)
                    .then_some(Self(Inner::Custom { weights }))
            }
        }
        /// Returns `None` if the specified length is zero
        pub fn new_equal(len: usize) -> Option<Self> {
            let max_index = len.checked_sub(1)?;
            Some(Self(Inner::Unity { max_index }))
        }
        pub fn get_max_index(self) -> usize {
            let Self(inner) = self;
            match inner {
                Inner::Unity { max_index } => max_index,
                Inner::Custom { weights } => weights.len() - 1,
            }
        }
        pub fn is_unity(self) -> bool {
            match self.0 {
                Inner::Unity { .. } => true,
                Inner::Custom { .. } => false,
            }
        }
        /// Gets the weight at the specified index
        ///
        /// # Panics
        /// Panics if the specified index is out of bounds
        pub fn get(self, index: usize) -> u32 {
            const EXPECT_IN_BOUNDS: &str = "index should be in bounds for Weights::get";

            let Self(inner) = self;
            match inner {
                Inner::Unity { max_index } => {
                    assert!(index <= max_index, "{EXPECT_IN_BOUNDS}");
                    1
                }
                Inner::Custom { weights } => {
                    assert!(index < weights.len(), "{EXPECT_IN_BOUNDS}");
                    weights[index]
                }
            }
        }
        /// Gets the weight at the specified index, as type `usize`
        ///
        /// # Panics
        /// Panics if the specified index is out of bounds, or the weight (`u32`) does not fit in
        /// the platform's `usize`.
        ///
        /// Note that the latter is unlikely for the use-case: when would you want to weight a node
        /// more heavily than there are addresses on the system? (e.g. system where `usize` is `u16`)
        pub fn get_as_usize(self, index: usize) -> usize {
            self.get(index)
                .try_into()
                .expect("weight should fit in platform's usize")
        }
    }
}
