pub mod atomic;
pub mod atomicflags;
pub mod semaphore;
pub mod spinlock;

// pub trait Synchronized {
//     fn synchronized<F, R>(&self, f: F) -> R
//     where
//         F: FnOnce() -> R;
// }
