// Semaphore

use super::signal::SignallingObject;
use crate::arch::cpu::Cpu;
use alloc::{boxed::Box, sync::Arc};
use core::{
    marker::PhantomData,
    pin::Pin,
    sync::atomic::*,
    task::{Context, Poll},
};
use futures_util::{task::AtomicWaker, Future};

/// counting semaphore
pub struct Semaphore {
    value: AtomicUsize,
    signal: SignallingObject,
}

impl Semaphore {
    #[inline]
    pub const fn new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            signal: SignallingObject::new(None),
        }
    }

    #[inline]
    pub fn estimated_value(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        Cpu::interlocked_fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        self.wait()
    }

    #[inline]
    pub fn unlock(&self) {
        self.signal()
    }

    #[inline]
    pub fn wait(&self) {
        self.signal.wait_for(|| self.try_lock());
    }

    #[inline]
    pub fn signal(&self) {
        let _ = Cpu::interlocked_increment(&self.value);
        let _ = self.signal.signal();
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

/// binary semaphore
pub struct BinarySemaphore {
    value: AtomicBool,
    signal: SignallingObject,
}

impl BinarySemaphore {
    #[inline]
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(false),
            signal: SignallingObject::new(None),
        }
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        self.value
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        self.signal.wait_for(|| self.try_lock())
    }

    #[inline]
    pub fn unlock(&self) {
        self.value.store(false, Ordering::Release);
        let _ = self.signal.signal();
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let result = f();
        self.unlock();
        result
    }
}

impl Default for BinarySemaphore {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct AsyncSemaphore {
    value: AtomicUsize,
    waker: AtomicWaker,
}

impl AsyncSemaphore {
    #[inline]
    pub fn new(value: usize) -> Pin<Arc<Self>> {
        Arc::pin(Self {
            value: AtomicUsize::new(value),
            waker: AtomicWaker::new(),
        })
    }

    #[inline]
    pub fn estimated_value(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn try_lock(&self) -> bool {
        Cpu::interlocked_fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    #[inline]
    pub fn wait(self: Pin<Arc<Self>>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(AsyncSemaphoreObserver { sem: self.clone() })
    }

    #[inline]
    pub fn wait_ok<T: 'static>(
        self: &Pin<Arc<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), T>>>> {
        Box::pin(AsyncSemaphoreResultObserver {
            sem: self.clone(),
            _phantom: PhantomData,
        })
    }

    #[must_use]
    pub fn poll(&self, cx: &mut Context<'_>) -> bool {
        self.waker.register(cx.waker());
        let result = self.try_lock();
        if result {
            self.waker.take();
        }
        result
    }

    #[inline]
    pub fn signal(&self) {
        let _ = Cpu::interlocked_increment(&self.value);
        let _ = self.waker.wake();
    }
}

struct AsyncSemaphoreObserver {
    sem: Pin<Arc<AsyncSemaphore>>,
}

impl Future for AsyncSemaphoreObserver {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.sem.poll(cx) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

struct AsyncSemaphoreResultObserver<T> {
    sem: Pin<Arc<AsyncSemaphore>>,
    _phantom: PhantomData<T>,
}

impl<T> Future for AsyncSemaphoreResultObserver<T> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.sem.poll(cx) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
