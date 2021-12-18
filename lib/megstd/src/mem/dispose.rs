//! Dispose

use core::ops::{Deref, DerefMut};

pub trait DisposeRef {
    fn dispose_ref(&mut self);
}

pub struct DisposableRef<'a, T: DisposeRef>(&'a mut T);

impl<'a, T: DisposeRef> DisposableRef<'a, T> {
    #[inline]
    pub fn new(val: &'a mut T) -> Self {
        Self(val)
    }
}

impl<T: DisposeRef> Drop for DisposableRef<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.0.dispose_ref()
    }
}

impl<T: DisposeRef> AsRef<T> for DisposableRef<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.0
    }
}

impl<T: DisposeRef> AsMut<T> for DisposableRef<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.0
    }
}

impl<T: DisposeRef> Deref for DisposableRef<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: DisposeRef> DerefMut for DisposableRef<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
