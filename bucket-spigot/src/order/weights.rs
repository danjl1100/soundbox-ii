// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

/// Non-empty weights (length non-zero, and contents non-zero)
#[derive(Clone, Copy, Debug)]
pub(super) struct Weights<'a>(Inner<'a>);

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
    /// Returns `None` if the specified slice is empty
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
    pub fn get(self, index: usize) -> u32 {
        let Self(inner) = self;
        match inner {
            Inner::Unity { max_index } => {
                assert!(
                    index <= max_index,
                    "index should be in bounds for Weights::get"
                );
                1
            }
            Inner::Custom { weights } => weights[index],
        }
    }
    pub fn get_as_usize(self, index: usize) -> usize {
        self.get(index)
            .try_into()
            .expect("weight should fit in platform's usize")
    }
}
