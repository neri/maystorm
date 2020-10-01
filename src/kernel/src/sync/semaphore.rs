// Semaphore

use super::atomic::*;
use crate::task::scheduler::*;
use core::sync::atomic::*;
use core::time::Duration;

pub struct Semaphore {
    value: AtomicIsize,
    signal_object: AtomicObject<SignallingObject>,
}

unsafe impl Sync for Semaphore {}

unsafe impl Send for Semaphore {}

impl Semaphore {
    pub const fn new(value: isize) -> Self {
        Self {
            value: AtomicIsize::new(value),
            signal_object: AtomicObject::NONE,
        }
    }

    pub fn try_to(&self) -> Result<(), ()> {
        let value = self.value.load(Ordering::Acquire);
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

    pub fn wait(&self, duration: Duration) -> Result<(), ()> {
        const MAX_DELTA: u64 = 7;
        let deadline = Timer::new(duration);
        loop {
            if self.try_to().is_ok() {
                return Ok(());
            } else {
                let mut delta: u64 = 1;
                loop {
                    let signal = SignallingObject::new();
                    if self.signal_object.cas(None, Some(signal)).is_ok() {
                        self.signal_object
                            .map(|signal| signal.wait(Duration::from_millis(delta)));
                        break;
                    } else {
                        MyScheduler::wait_for(None, Duration::from_millis(delta));
                    }
                    if !deadline.until() {
                        return Err(());
                    }
                    delta = core::cmp::min(delta << 1, MAX_DELTA);
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
}
