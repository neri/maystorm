//! Concurrent First In First Out

use crate::arch::cpu::Cpu;
use crate::sync::spinlock::SpinLoopWait;
use alloc::{boxed::Box, vec::Vec};
use core::{
    mem::{self, MaybeUninit},
    {cell::UnsafeCell, sync::atomic::*},
};

/// Concurrent First In First Out
pub struct ConcurrentFifo<T> {
    head: AtomicUsize,
    tail: AtomicUsize,
    mask: usize,
    one_lap: usize,
    data: *mut Slot<T>,
}

unsafe impl<T: Send> Send for ConcurrentFifo<T> {}

unsafe impl<T: Send> Sync for ConcurrentFifo<T> {}

impl<T: Sized> ConcurrentFifo<T> {
    #[inline]
    pub fn with_capacity(size: usize) -> Self {
        let capacity = (size + 1).next_power_of_two();
        let mask = capacity - 1;

        let data = {
            let mut boxed: Box<[Slot<T>]> = (0..capacity).map(|v| Slot::new(v)).collect();
            let ptr = boxed.as_mut_ptr();
            mem::forget(boxed);
            ptr
        };

        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            mask,
            one_lap: capacity,
            data,
        }
    }

    pub fn enqueue(&self, value: T) -> Result<(), T> {
        unsafe { Cpu::without_interrupts(|| self._enqueue(value)) }
    }

    pub fn dequeue(&self) -> Option<T> {
        unsafe { Cpu::without_interrupts(|| self._dequeue()) }
    }

    #[inline]
    fn _enqueue(&self, data: T) -> Result<(), T> {
        let mut spin = SpinLoopWait::new();
        loop {
            let tail = self.tail.load(Ordering::Relaxed);
            if (tail + 1) & self.mask == self.head.load(Ordering::Relaxed) & self.mask {
                return Err(data);
            }
            let index = tail & self.mask;
            let slot = unsafe { &mut *self.data.add(index) };
            if slot.stamp() == tail {
                let new_tail = tail.wrapping_add(1);
                match self.tail.compare_exchange_weak(
                    tail,
                    new_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        slot.write(data, new_tail);
                        return Ok(());
                    }
                    Err(_) => (),
                }
            }
            spin.wait();
        }
    }

    #[inline]
    fn _dequeue(&self) -> Option<T> {
        let mut spin = SpinLoopWait::new();
        loop {
            let head = self.head.load(Ordering::Relaxed);
            let index = head & self.mask;
            if index == self.tail.load(Ordering::Relaxed) & self.mask {
                return None;
            }
            let slot = unsafe { &*self.data.add(index) };
            let new_head = head.wrapping_add(1);
            if slot.stamp() == new_head {
                match self.head.compare_exchange_weak(
                    head,
                    new_head,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let data = slot.read();
                        slot.write_stamp(head + self.one_lap);
                        return Some(data);
                    }
                    Err(_) => (),
                }
            }
            spin.wait();
        }
    }
}

impl<T> Drop for ConcurrentFifo<T> {
    fn drop(&mut self) {
        while let Some(t) = self._dequeue() {
            drop(t);
        }
        unsafe {
            Vec::from_raw_parts(self.data, 0, self.one_lap);
        }
    }
}

struct Slot<T> {
    stamp: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Slot<T> {
    #[inline]
    fn new(stamp: usize) -> Self {
        Self {
            stamp: AtomicUsize::new(stamp),
            value: UnsafeCell::new(MaybeUninit::zeroed()),
        }
    }

    #[inline]
    fn stamp(&self) -> usize {
        self.stamp.load(Ordering::Acquire)
    }

    #[inline]
    fn write_stamp(&self, val: usize) {
        self.stamp.store(val, Ordering::Release);
    }

    #[inline]
    fn write(&mut self, val: T, stamp: usize) {
        self.value.get_mut().write(val);
        self.write_stamp(stamp);
    }

    #[inline]
    fn read(&self) -> T {
        fence(Ordering::SeqCst);
        unsafe { mem::transmute_copy(&*self.value.get()) }
    }
}
