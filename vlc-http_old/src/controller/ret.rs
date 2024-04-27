// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper zero-sized types to facilitate optionally returning a value
//! This allows cloning only when needed, as the clone only occurs when [`Some`] value is requested.

mod private {
    pub trait Sealed {}
    impl Sealed for super::Some {}
    impl Sealed for super::None {}
}
pub trait Returner<T>: private::Sealed {
    type Return;
    /// Applies the data to the `observer`, then returns
    fn apply_with<F>(t: T, observer: F) -> Self::Return
    where
        F: FnOnce(T);
}
/// Some return data is requested
pub enum Some {}
impl<T: Clone> Returner<T> for Some {
    type Return = T;
    fn apply_with<F>(t: T, observer: F) -> T
    where
        F: FnOnce(T),
    {
        observer(t.clone());
        t
    }
}
/// Return data is not needed
pub enum None {}
impl<T> Returner<T> for None {
    type Return = ();
    fn apply_with<F>(t: T, observer: F)
    where
        F: FnOnce(T),
    {
        observer(t);
    }
}
