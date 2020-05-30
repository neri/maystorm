// Semaphore

use core::sync::atomic::*;
// use crate::myos::scheduler::*;

pub struct Semaphore {
    value: AtomicIsize,
    signal_object: Option<usize>,
}

impl Semaphore {
    pub fn new(value: isize) -> Self {
        Self {
            value: AtomicIsize::new(value),
            signal_object: None,
        }
    }

    pub fn wait(&self) {
        loop {
            let value = self.value.load(Ordering::Acquire);
            if value >= 1 {
                match self.value.compare_exchange(
                    value,
                    value - 1,
                    Ordering::SeqCst,
                    Ordering::Release,
                ) {
                    Ok(_) => break,
                    Err(_) => (),
                }
            }
        }
    }

    pub fn signal(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.wait();
        let result = f();
        self.signal();
        result
    }
}
