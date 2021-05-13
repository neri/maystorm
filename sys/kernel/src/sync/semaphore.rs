// Semaphore

use crate::arch::cpu::Cpu;
use crate::task::scheduler::*;
use core::sync::atomic::*;
use core::time::Duration;

pub struct Semaphore {
    value: AtomicUsize,
    signal_object: AtomicUsize,
}

impl Semaphore {
    #[inline]
    pub const fn new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            signal_object: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn try_to(&self) -> bool {
        Cpu::interlocked_fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    pub fn wait(&self) {
        const MAX_DELTA: u64 = 7;
        loop {
            if self.try_to() {
                return;
            } else {
                let mut delta: u64 = 0;
                loop {
                    let current = Scheduler::current_thread().unwrap();
                    if Cpu::interlocked_compare_and_swap(&self.signal_object, 0, current.as_usize())
                        .0
                    {
                        Scheduler::sleep();
                        if self.try_to() {
                            return;
                        }
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
        let _ = Cpu::interlocked_increment(&self.value);
        if let Some(thread) = ThreadHandle::new(Cpu::interlocked_swap(&self.signal_object, 0)) {
            thread.wake();
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
