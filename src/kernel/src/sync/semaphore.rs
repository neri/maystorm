// Semaphore

use super::atomic::*;
use crate::task::scheduler::*;
use core::sync::atomic::*;
use core::time::Duration;

pub struct Semaphore {
    value: AtomicIsize,
    signal_object: AtomicObject<SignallingObject>,
}

impl Semaphore {
    pub const fn new(value: isize) -> Self {
        Self {
            value: AtomicIsize::new(value),
            signal_object: AtomicObject::NONE,
        }
    }

    pub fn try_to(&self) -> Result<(), ()> {
        let value = self.value.load(Ordering::Relaxed);
        if value >= 1
            && self
                .value
                .compare_exchange(value, value - 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
        {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn wait(&self) {
        const MAX_DELTA: u64 = 7;
        loop {
            if self.try_to().is_ok() {
                return;
            } else {
                let mut delta: u64 = 0;
                loop {
                    let signal = SignallingObject::new();
                    if self.signal_object.cas(None, Some(signal)).is_ok() {
                        self.signal_object.map(|signal| {
                            signal.wait(Duration::from_millis(0));
                            let _ = self.signal_object.cas(Some(signal), None);
                        });
                        break;
                    } else {
                        Timer::sleep(Duration::from_millis(1 << delta));
                    }
                    if delta < MAX_DELTA {
                        delta += 1;
                    }
                }
            }
        }
    }

    pub fn signal(&self) {
        let old_value = self.value.fetch_add(1, Ordering::SeqCst);
        if old_value == 0 {
            if let Some(signal) = self.signal_object.swap(None) {
                signal.signal();
            }
        }
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
