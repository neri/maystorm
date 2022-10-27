use super::pci::PciConfigAddress;
use core::sync::atomic::AtomicUsize;

pub use crate::arch::hal::{Hal, InterruptGuard, Spinlock};

pub trait HalTrait {
    fn cpu() -> impl HalCpu;

    fn pci() -> impl HalPci;

    fn spin_loop() -> impl HalSpinLoopWait;
}

pub trait HalCpu {
    fn reset(&self) -> !;

    fn stop(&self) -> !;

    fn spin_loop_hint(&self);

    unsafe fn wait_for_interrupt(&self);

    unsafe fn enable_interrupt(&self);

    unsafe fn disable_interrupt(&self);

    fn interlocked_increment(&self, p: &AtomicUsize) -> usize;

    fn interlocked_compare_and_swap(
        &self,
        p: &AtomicUsize,
        current: usize,
        new: usize,
    ) -> (bool, usize);

    fn interlocked_fetch_update<F>(&self, p: &AtomicUsize, f: F) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>;

    fn interlocked_swap(&self, p: &AtomicUsize, val: usize) -> usize;

    fn interlocked_test_and_set(&self, p: &AtomicUsize, position: usize) -> bool;

    fn interlocked_test_and_clear(&self, p: &AtomicUsize, position: usize) -> bool;

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
