//! First In First Out

use crate::arch::cpu::Cpu;
use crossbeam_queue::ArrayQueue;

/// FIFO wrapper
#[repr(transparent)]
pub struct ConcurrentFifo<T>(ArrayQueue<T>);

unsafe impl<T: Send> Send for ConcurrentFifo<T> {}

unsafe impl<T: Send> Sync for ConcurrentFifo<T> {}

impl<T> ConcurrentFifo<T> {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self(ArrayQueue::new(cap))
    }

    #[inline]
    pub fn enqueue(&self, value: T) -> Result<(), T> {
        unsafe { Cpu::without_interrupts(|| self.0.push(value)) }
    }

    #[inline]
    pub fn dequeue(&self) -> Option<T> {
        unsafe { Cpu::without_interrupts(|| self.0.pop()) }
    }
}
