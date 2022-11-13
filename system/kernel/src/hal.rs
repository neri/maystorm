use crate::{drivers::pci::PciConfigAddress, system::ProcessorIndex};
use core::{
    num::NonZeroU64,
    ops::{Add, BitAnd, BitOr, Mul, Not, Sub},
    sync::atomic::{AtomicUsize, Ordering},
};

pub use crate::arch::hal::{Hal, InterruptGuard, Spinlock};

#[const_trait]
pub trait HalTrait {
    fn cpu() -> impl HalCpu;

    fn sync() -> impl HalSync;

    fn pci() -> impl HalPci;
}

pub trait HalCpu {
    fn current_processor_index(&self) -> ProcessorIndex;

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

    #[must_use]
    unsafe fn interrupt_guard(&self) -> InterruptGuard;

    fn spin_wait(&self) -> impl HalSpinLoopWait;
}

pub trait HalSync {
    #[inline]
    fn fetch_inc(&self, ptr: &AtomicUsize) -> usize {
        ptr.fetch_add(1, Ordering::SeqCst)
    }

    #[inline]
    fn compare_and_swap(&self, ptr: &AtomicUsize, current: usize, new: usize) -> (bool, usize) {
        match ptr.compare_exchange(current, new, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(v) => (true, v),
            Err(v) => (false, v),
        }
    }

    #[inline]
    fn fetch_update<F>(&self, ptr: &AtomicUsize, f: F) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>,
    {
        ptr.fetch_update(Ordering::SeqCst, Ordering::Relaxed, f)
    }

    #[inline]
    fn swap(&self, ptr: &AtomicUsize, val: usize) -> usize {
        ptr.swap(val, Ordering::SeqCst)
    }

    #[inline]
    fn test_and_set(&self, ptr: &AtomicUsize, position: usize) -> bool {
        let bit = 1 << position;
        (ptr.fetch_or(bit, Ordering::SeqCst) & bit) != 0
    }

    #[inline]
    fn test_and_clear(&self, ptr: &AtomicUsize, position: usize) -> bool {
        let bit = 1 << position;
        (ptr.fetch_and(!bit, Ordering::SeqCst) & bit) != 0
    }
}

pub trait HalPci {
    unsafe fn read(&self, addr: PciConfigAddress) -> u32;

    unsafe fn write(&self, addr: PciConfigAddress, value: u32);

    unsafe fn register_msi(&self, f: fn(usize) -> (), val: usize) -> Result<(u64, u16), ()>;
}

pub trait HalSpinlock {
    #[must_use]
    fn try_lock(&self) -> bool;

    fn lock(&self);

    unsafe fn force_unlock(&self);

    #[inline]
    fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let result = f();
        unsafe {
            self.force_unlock();
        }
        result
    }
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

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub const NULL: Self = Self(0);

    #[inline]
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn from_usize(val: usize) -> Self {
        Self(val as u64)
    }

    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Gets a pointer identical to the specified physical address.
    ///
    /// # Safety
    ///
    /// Pointers of this form may not map to some memory.
    #[inline]
    pub const unsafe fn identity_map<T>(&self) -> *mut T {
        self.0 as usize as *mut T
    }
}

impl Default for PhysicalAddress {
    #[inline]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl const Add<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs as u64)
    }
}

impl const Add<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl const Sub<PhysicalAddress> for PhysicalAddress {
    type Output = usize;

    #[inline]
    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        (self.0 - rhs.0) as usize
    }
}

impl const Sub<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs as u64)
    }
}

impl const Mul<usize> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self::Output {
        Self(self.0 * rhs as u64)
    }
}

impl const Mul<u64> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl const BitAnd<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: u64) -> Self::Output {
        Self(self.0 & rhs)
    }
}

impl const BitAnd<PhysicalAddress> for u64 {
    type Output = Self;

    fn bitand(self, rhs: PhysicalAddress) -> Self::Output {
        self & rhs.0
    }
}

impl const BitOr<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: u64) -> Self::Output {
        Self(self.0 | rhs)
    }
}

impl const Not for PhysicalAddress {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl const From<u64> for PhysicalAddress {
    #[inline]
    fn from(val: u64) -> Self {
        Self::new(val)
    }
}

impl const From<PhysicalAddress> for u64 {
    #[inline]
    fn from(val: PhysicalAddress) -> Self {
        val.as_u64()
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonNullPhysicalAddress(NonZeroU64);

impl NonNullPhysicalAddress {
    #[inline]
    pub const fn get(&self) -> PhysicalAddress {
        PhysicalAddress(self.0.get())
    }

    #[inline]
    pub const fn new(val: PhysicalAddress) -> Option<Self> {
        match NonZeroU64::new(val.as_u64()) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const unsafe fn new_unchecked(val: PhysicalAddress) -> Self {
        Self(NonZeroU64::new_unchecked(val.as_u64()))
    }
}

impl const From<NonNullPhysicalAddress> for PhysicalAddress {
    #[inline]
    fn from(val: NonNullPhysicalAddress) -> Self {
        val.get()
    }
}
