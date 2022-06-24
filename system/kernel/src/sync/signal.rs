use crate::task::scheduler::*;
use core::{sync::atomic::*, time::Duration};

/// Signalling Object
#[repr(transparent)]
pub struct SignallingObject {
    data: AtomicUsize,
}

impl SignallingObject {
    #[inline]
    const fn _new(t: Option<ThreadHandle>) -> Self {
        Self {
            data: AtomicUsize::new(Self::_from_t(t)),
        }
    }

    #[inline]
    const fn _from_t(val: Option<ThreadHandle>) -> usize {
        match val {
            Some(v) => v.as_usize(),
            None => 0,
        }
    }

    #[inline]
    const fn _into_t(val: usize) -> Option<ThreadHandle> {
        ThreadHandle::new(val)
    }

    #[inline]
    pub fn load(&self) -> Option<ThreadHandle> {
        Self::_into_t(self.data.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn swap(&self, val: Option<ThreadHandle>) -> Option<ThreadHandle> {
        Self::_into_t(self.data.swap(Self::_from_t(val), Ordering::SeqCst))
    }

    #[inline]
    pub fn compare_and_swap(
        &self,
        current: Option<ThreadHandle>,
        new: Option<ThreadHandle>,
    ) -> Result<Option<ThreadHandle>, Option<ThreadHandle>> {
        match self.data.compare_exchange(
            Self::_from_t(current),
            Self::_from_t(new),
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(v) => Ok(Self::_into_t(v)),
            Err(v) => Err(Self::_into_t(v)),
        }
    }

    #[inline]
    pub fn wait_for<F>(&self, mut f: F)
    where
        F: FnMut() -> bool,
    {
        // TODO: wait queue
        const MAX_DELTA: u64 = 7;
        loop {
            if f() {
                return;
            } else {
                let mut delta = 0;
                loop {
                    if self.sleep().is_ok() {
                        if f() {
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

    #[inline]
    fn sleep(&self) -> Result<Option<ThreadHandle>, Option<ThreadHandle>> {
        let current = Scheduler::current_thread().unwrap();
        match self.compare_and_swap(None, Some(current)) {
            Ok(v) => {
                Scheduler::sleep();
                Ok(v)
            }
            Err(v) => Err(v),
        }
    }

    #[inline]
    pub fn signal(&self) -> Option<()> {
        self.swap(None).map(|thread| thread.wake())
    }
}

impl const Default for SignallingObject {
    #[inline]
    fn default() -> Self {
        Self::_new(None)
    }
}
