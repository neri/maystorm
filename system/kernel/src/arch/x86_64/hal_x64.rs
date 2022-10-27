pub mod hal {

    use crate::{
        arch::{
            apic::Apic,
            cpu::{Cpu, Rflags},
        },
        drivers::pci::PciConfigAddress,
        hal::*,
        task::scheduler::Scheduler,
        *,
    };
    use core::{
        arch::asm,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    pub struct Hal;

    impl HalTrait for Hal {
        #[inline]
        fn cpu() -> impl HalCpu {
            CpuImpl
        }

        #[inline]
        fn pci() -> impl HalPci {
            PciImpl
        }

        #[inline]
        fn spin_loop() -> impl HalSpinLoopWait {
            SpinLoopWait::new()
        }
    }

    struct CpuImpl;

    impl HalCpu for CpuImpl {
        #[inline]
        fn no_op(&self) {
            unsafe {
                asm!("nop", options(nomem, nostack));
            }
        }

        #[inline]
        fn spin_loop_hint(&self) {
            unsafe {
                asm!("pause", options(nomem, nostack));
            }
        }

        #[inline]
        unsafe fn wait_for_interrupt(&self) {
            asm!("hlt", options(nomem, nostack));
        }

        #[inline]
        unsafe fn enable_interrupt(&self) {
            asm!("sti", options(nomem, nostack));
        }

        #[inline]
        unsafe fn disable_interrupt(&self) {
            asm!("cli", options(nomem, nostack));
        }

        fn reset(&self) -> ! {
            unsafe {
                self.disable_interrupt();
                let _ = Scheduler::freeze(true);

                Cpu::out8(0x0CF9, 0x06);
                asm!("out 0x92, al", in("al") 0x01 as u8, options(nomem, nostack));
            }

            self.stop();
        }

        #[inline]
        fn interlocked_test_and_set(&self, p: &AtomicUsize, position: usize) -> bool {
            unsafe {
                let p = p as *const _ as *mut usize;
                let r: usize;
                asm!("
                    lock bts [{0}], {1}
                    sbb {2}, {2}
                    ", in(reg) p, in(reg) position, lateout(reg) r);
                r != 0
            }
        }

        #[inline]
        fn interlocked_test_and_clear(&self, p: &AtomicUsize, position: usize) -> bool {
            unsafe {
                let p = p as *const _ as *mut usize;
                let r: usize;
                asm!("
                    lock btr [{0}], {1}
                    sbb {2}, {2}
                    ", in(reg) p, in(reg) position, lateout(reg) r);
                r != 0
            }
        }

        #[inline]
        unsafe fn interrupt_guard(&self) -> InterruptGuard {
            let mut rax: u64;
            asm!("
                pushfq
                cli
                pop {0}
                ", lateout(reg) rax);
            InterruptGuard(rax)
        }
    }

    #[must_use]
    pub struct InterruptGuard(u64);

    impl Drop for InterruptGuard {
        fn drop(&mut self) {
            if Rflags::from_bits_retain(self.0).contains(Rflags::IF) {
                unsafe {
                    Hal::cpu().enable_interrupt();
                }
            }
        }
    }

    struct PciImpl;

    impl HalPci for PciImpl {
        #[inline]
        unsafe fn read(&self, addr: crate::drivers::pci::PciConfigAddress) -> u32 {
            without_interrupts!({
                Cpu::out32(0xCF8, addr.into());
                Cpu::in32(0xCFC)
            })
        }

        #[inline]
        unsafe fn write(&self, addr: crate::drivers::pci::PciConfigAddress, value: u32) {
            without_interrupts!({
                Cpu::out32(0xCF8, addr.into());
                Cpu::out32(0xCFC, value);
            })
        }

        #[inline]
        unsafe fn register_msi(&self, f: fn(usize) -> (), val: usize) -> Result<(u64, u16), ()> {
            Apic::register_msi(f, val)
        }
    }

    impl Into<u32> for PciConfigAddress {
        #[inline]
        fn into(self) -> u32 {
            0x8000_0000
                | ((self.get_bus() as u32) << 16)
                | ((self.get_dev() as u32) << 11)
                | ((self.get_fun() as u32) << 8)
                | ((self.get_register() as u32) << 2)
        }
    }

    #[derive(Default)]
    pub struct Spinlock {
        value: AtomicBool,
    }

    impl Spinlock {
        const LOCKED_VALUE: bool = true;
        const UNLOCKED_VALUE: bool = false;

        #[inline]
        pub const fn new() -> Self {
            Self {
                value: AtomicBool::new(Self::UNLOCKED_VALUE),
            }
        }

        #[inline]
        #[must_use]
        pub fn try_lock(&self) -> bool {
            self.value
                .compare_exchange(
                    Self::UNLOCKED_VALUE,
                    Self::LOCKED_VALUE,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
        }

        pub fn lock(&self) {
            while self
                .value
                .compare_exchange(
                    Self::UNLOCKED_VALUE,
                    Self::LOCKED_VALUE,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                let mut spin_loop = SpinLoopWait::new();
                while self.value.load(Ordering::Relaxed) {
                    spin_loop.wait();
                }
            }
        }

        #[inline]
        pub unsafe fn force_unlock(&self) {
            self.value.store(Self::UNLOCKED_VALUE, Ordering::Release);
        }

        #[inline]
        pub fn synchronized<F, R>(&self, f: F) -> R
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

    struct SpinLoopWait(usize);

    impl SpinLoopWait {
        #[inline]
        pub const fn new() -> Self {
            Self(0)
        }
    }

    impl HalSpinLoopWait for SpinLoopWait {
        #[inline]
        fn reset(&mut self) {
            self.0 = 0;
        }

        fn wait(&mut self) {
            let count = self.0;
            for _ in 0..(1 << count) {
                Hal::cpu().spin_loop_hint();
            }
            if count < 6 {
                self.0 += 1;
            }
        }
    }
}
