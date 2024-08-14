// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

// `Option<Lazy<T>>` seems cleaner and more meaningful than `Option<Option<T>>`
// (heeding advice from the pedantic lint `clippy::option_option`)
#[derive(Clone, Default)]
pub(super) enum Lazy<T> {
    Value(T),
    #[default]
    Uninit,
}
impl<T> Lazy<T> {
    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Uninit => None,
        }
    }
    pub fn as_mut_or_init(&mut self, init_fn: impl FnOnce() -> T) -> &mut T {
        match self {
            Self::Value(_) => {}
            Self::Uninit => {
                *self = Self::Value(init_fn());
            }
        }
        self.as_mut().expect("should initialize directly above")
    }
}

#[derive(Clone)]
pub(super) struct CountsRemaining(Vec<Option<Lazy<Self>>>);
impl CountsRemaining {
    pub fn new(len: usize) -> Self {
        Self(vec![Some(Lazy::default()); len])
    }
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    pub fn set_empty(&mut self, index: usize) {
        self.0[index].take();

        // check if all are exhausted
        if self.0.iter().all(Option::is_none) {
            // ensure any future calls error (loudly)
            self.0.clear();
        }
    }
    /// Returns a mutable reference to the child's remaining count (which may not yet be
    /// initialized) or `None` if the child is exhausted (e.g. via [`Self::set_empty`])
    ///
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    pub fn child_mut(&mut self, index: usize) -> Option<&mut Lazy<Self>> {
        self.0[index].as_mut()
    }
    /// Returns true if all children are exhausted
    pub fn is_fully_exhausted(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns the number of children, or `0` if all children are exhausted
    pub fn child_count_if_nonempty(&self) -> usize {
        self.0.len()
    }
}
