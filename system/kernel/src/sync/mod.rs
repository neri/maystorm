//! Classes to synchronize

pub mod fifo;
pub mod semaphore;
pub mod signal;
pub mod spinlock;

pub mod atomic {
    mod atomicenum;
    mod atomicflags;
    mod atomicfloat;
    pub use atomicenum::*;
    pub use atomicflags::*;
    pub use atomicfloat::*;
}

mod mutex;
pub use mutex::*;
mod rwlock;
pub use rwlock::*;

use core::fmt;

pub type LockResult<Guard> = Result<Guard, PoisonError<Guard>>;
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// NOT YET IMPLEMENTED
#[allow(dead_code)]
pub struct PoisonError<T> {
    guard: T,
}

impl<T> fmt::Debug for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "PoisonError { inner: .. }".fmt(f)
    }
}

pub enum TryLockError<T> {
    Poisoned(PoisonError<T>),
    WouldBlock,
}

impl<T> From<PoisonError<T>> for TryLockError<T> {
    #[inline]
    fn from(err: PoisonError<T>) -> TryLockError<T> {
        TryLockError::Poisoned(err)
    }
}
