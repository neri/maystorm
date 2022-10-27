use crate::drivers::pci::PciConfigAddress;
use core::sync::atomic::{AtomicUsize, Ordering};

pub use crate::arch::hal::{Hal, InterruptGuard, Spinlock};

pub trait HalTrait {
    fn cpu() -> impl HalCpu;

    fn pci() -> impl HalPci;

    fn spin_loop() -> impl HalSpinLoopWait;
}

pub trait HalCpu {
    fn no_op(&self);

    fn spin_loop_hint(&self);

    unsafe fn wait_for_interrupt(&self);

    unsafe fn enable_interrupt(&self);

    unsafe fn disable_interrupt(&self);

    fn reset(&self) -> !;

    #[inline]
    fn stop(&self) -> ! {
        loop {
            unsafe {
                self.disable_interrupt();
                self.wait_for_interrupt();
            }
        }
    }

    #[inline]
    fn interlocked_increment(&self, ptr: &AtomicUsize) -> usize {
        ptr.fetch_add(1, Ordering::SeqCst)
    }

    #[inline]
    fn interlocked_compare_and_swap(
        &self,
        ptr: &AtomicUsize,
        current: usize,
        new: usize,
    ) -> (bool, usize) {
        match ptr.compare_exchange(current, new, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(v) => (true, v),
            Err(v) => (false, v),
        }
    }

    fn interlocked_fetch_update<F>(&self, ptr: &AtomicUsize, f: F) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>,
    {
        ptr.fetch_update(Ordering::SeqCst, Ordering::Relaxed, f)
    }

    fn interlocked_swap(&self, ptr: &AtomicUsize, val: usize) -> usize {
        ptr.swap(val, Ordering::SeqCst)
    }

    fn interlocked_test_and_set(&self, ptr: &AtomicUsize, position: usize) -> bool;

    fn interlocked_test_and_clear(&self, ptr: &AtomicUsize, position: usize) -> bool;

    #[must_use]
    unsafe fn interrupt_guard(&self) -> InterruptGuard;
}

pub trait HalPci {
    unsafe fn read(&self, addr: PciConfigAddress) -> u32;

    unsafe fn write(&self, addr: PciConfigAddress, value: u32);

    unsafe fn register_msi(&self, f: fn(usize) -> (), val: usize) -> Result<(u64, u16), ()>;
}

pub trait HalSpinLoopWait {
    fn reset(&mut self);

    fn wait(&mut self);
}

#[macro_export]
macro_rules! without_interrupts {
    ( $f:expr ) => {{
        let rflags = Hal::cpu().interrupt_guard();
        let r = { $f };
        drop(rflags);
        r
    }};
}
