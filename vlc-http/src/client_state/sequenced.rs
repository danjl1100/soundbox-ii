// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use sequence::Instance;
pub(crate) use sequence::Sequence;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub(crate) struct Sequenced<T> {
    inner: T,
    sequence: Sequence,
}
impl<T> Sequenced<T> {
    fn new(inner: T, instance: Instance) -> Self {
        Self {
            inner,
            sequence: Sequence::new(instance),
        }
    }
    pub fn get_sequence(&self) -> Sequence {
        self.sequence
    }
    fn increment(&mut self) {
        self.sequence = self.sequence.next();
    }
    // TODO delete if unused
    // pub fn modify<U>(&mut self, modify_fn: impl Fn(&mut T) -> U) -> U {
    //     self.increment();
    //     (modify_fn)(&mut self.inner)
    // }
    pub fn replace(&mut self, new: T) -> T {
        self.increment();
        std::mem::replace(&mut self.inner, new)
    }
}
impl<T> std::ops::Deref for Sequenced<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Copy)]
pub struct Builder(Instance);
impl Sequenced<()> {
    pub fn builder() -> Builder {
        Builder(Instance::new())
    }
}
impl Builder {
    pub fn next_default<T>(self) -> Sequenced<T>
    where
        T: Default,
    {
        let inner = Default::default();
        self.next(inner)
    }
    pub fn next<T>(self, inner: T) -> Sequenced<T> {
        let Self(instance) = self;
        Sequenced::new(inner, instance)
    }
}

mod sequence {
    pub(super) use instance::Instance;

    /// Sequential marker for a specific [`Instance`]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct Sequence {
        instance: Instance,
        count: u64,
    }
    impl Sequence {
        pub fn new(instance: Instance) -> Self {
            Self { instance, count: 0 }
        }
        pub fn next(self) -> Self {
            let Self { instance, count } = self;
            Self {
                instance,
                count: count + 1,
            }
        }
        #[allow(unused)] // TODO remove if unused?
        pub fn max(self, other: Self) -> Option<Self> {
            self.partial_cmp(&other).map(|ord| match ord {
                // self >= other
                std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => self,
                // self < other
                std::cmp::Ordering::Less => other,
            })
        }
    }
    impl PartialOrd for Sequence {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            let Self { instance, count } = *self;
            (instance == other.instance).then_some(count.cmp(&other.count))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn identity() {
            let obj1 = Instance::new();
            let count1 = Sequence::new(obj1);

            let obj2 = Instance::new();
            let count2 = Sequence::new(obj2);

            assert_ne!(count1, count2);
        }
        #[test]
        fn partial_ord() {
            let obj1 = Instance::new();
            let count1 = Sequence::new(obj1);

            let count1_copy = count1;
            assert_eq!(count1_copy, count1);

            let count1_next = count1.next();
            assert_ne!(count1_copy, count1_next);
            assert_eq!(count1_copy.next(), count1_next);
            assert_eq!(count1_next.max(count1), Some(count1_next));

            assert!(count1_copy < count1_next);
            assert!(count1_copy.next().next() > count1_next);

            let obj2 = Instance::new();
            let count2 = Sequence::new(obj2);

            assert_ne!(count1.next(), count2.next());

            assert_eq!(count1.max(count2), None);
        }
    }

    mod instance {
        use std::sync::atomic::AtomicU64;

        static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

        /// A globally-unique identifier
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        // NOTE: PartialOrd/Ord does not make sense
        pub(crate) struct Instance(u64);
        impl Instance {
            pub fn new() -> Self {
                let counter = GLOBAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Self(counter)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            #[test]
            fn identity() {
                let obj1 = Instance::new();
                let obj2 = Instance::new();

                assert_eq!(obj1, obj1);
                assert_eq!(obj2, obj2);

                assert_ne!(obj1, obj2);
                assert_ne!(obj2, obj1);

                let obj1_copy = obj1;
                assert_eq!(obj1_copy, obj1);
                assert_ne!(obj1_copy, obj2);
            }
        }
    }
}
