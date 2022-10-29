pub mod hal {

    use crate::{
        arch::cpu::Cpu, drivers::pci::PciConfigAddress, hal::*, system::ProcessorIndex,
        task::scheduler::Scheduler, *,
    };
    use core::{
        arch::asm,
        sync::atomic::{AtomicU32, AtomicUsize, Ordering},
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
        fn spin_wait() -> impl HalSpinLoopWait {
            SpinLoopWait::new()
        }
    }

    impl HalCpu for CpuImpl {
        #[inline]
        fn current_processor_index(&self) -> ProcessorIndex {
            // TODO:
            ProcessorIndex(0)
        }

        #[inline]
        fn no_op(&self) {
            unsafe {
                asm!("nop", options(nomem, nostack));
            }
        }

        #[inline]
        fn spin_loop_hint(&self) {
            unsafe {
                asm!("sevl\nwfe", options(nomem, nostack));
            }
        }

        #[inline]
        fn wait_for_interrupt(&self) {
            unsafe {
                asm!("wfi", options(nomem, nostack));
            }
        }

        #[inline]
        unsafe fn enable_interrupt(&self) {
            asm!("
                mrs {0}, daif
                bic {0}, {0}, #0x3C0
                msr daif, {0}
                ", out(reg)_, options(nomem, nostack));
        }

        #[inline]
        unsafe fn disable_interrupt(&self) {
            asm!("
                mrs {0}, daif
                orr {0}, {0}, #0x3C0
                msr daif, {0}
                ", out(reg)_, options(nomem, nostack));
        }

        #[inline]
        unsafe fn interrupt_guard(&self) -> InterruptGuard {
            let old: usize;
            asm!("
                mrs {0}, daif
                orr {1}, {0}, #0x3C0
                msr daif, {1}
                ", out(reg)old, out(reg)_, options(nomem, nostack));
            InterruptGuard(old)
        }
    }

    #[must_use]
    pub struct InterruptGuard(usize);

    impl !Send for InterruptGuard {}

    impl !Sync for InterruptGuard {}

    impl Drop for InterruptGuard {
        fn drop(&mut self) {
            if (self.0 & 0x3C0) != 0 {
                unsafe {
                    Cpu::enable_interrupt();
                }
            }
        }
    }

    struct PciImpl;

    impl HalPci for PciImpl {
        #[inline]
        unsafe fn read(&self, addr: crate::drivers::pci::PciConfigAddress) -> u32 {
            todo!();
        }

        #[inline]
        unsafe fn write(&self, addr: crate::drivers::pci::PciConfigAddress, value: u32) {
            todo!();
        }

        #[inline]
        unsafe fn register_msi(&self, f: fn(usize) -> (), val: usize) -> Result<(u64, u16), ()> {
            todo!();
        }
    }

    //     impl Into<u32> for PciConfigAddress {
    //         #[inline]
    //         fn into(self) -> u32 {
    //             0x8000_0000
    //                 | ((self.get_bus() as u32) << 16)
    //                 | ((self.get_dev() as u32) << 11)
    //                 | ((self.get_fun() as u32) << 8)
    //                 | ((self.get_register() as u32) << 2)
    //         }
    //     }

    #[derive(Default)]
    pub struct Spinlock {
        value: AtomicU32,
    }

    impl Spinlock {
        pub const LOCKED_VALUE: u32 = 1;
        pub const UNLOCKED_VALUE: u32 = 0;

        #[inline]
        pub const fn new() -> Self {
            Self {
                value: AtomicU32::new(Self::UNLOCKED_VALUE),
            }
        }
    }

    impl HalSpinlock for Spinlock {
        #[must_use]
        fn try_lock(&self) -> bool {
            let result: u32;
            unsafe {
                asm!("
                    ldaxr {0:w}, [{1}]
                    cbnz {0:w}, 1f
                    stxr {0:w}, {2:w}, [{1}]
                1:
                ", out(reg)result, in(reg)&self.value, in(reg)Self::LOCKED_VALUE);
            }
            result == 0
        }

        fn lock(&self) {
            unsafe {
                asm!("
                        sevl
                    1:  wfe
                    2:  ldaxr {0:w}, [{1}]
                        cbnz {0:w}, 1b
                        stxr {0:w}, {2:w}, [{1}]
                        cbnz {0:w}, 2b
                    ", out(reg)_, in(reg)&self.value, in(reg)Self::LOCKED_VALUE);
            }
        }

        #[inline]
        unsafe fn force_unlock(&self) {
            self.value.store(Self::UNLOCKED_VALUE, Ordering::Release);
        }
    }

    #[derive(Debug, Default)]
    pub struct SpinLoopWait;

    impl SpinLoopWait {
        #[inline]
        pub const fn new() -> Self {
            Self {}
        }
    }

    impl HalSpinLoopWait for SpinLoopWait {
        #[inline]
        fn reset(&mut self) {}

        fn wait(&mut self) {
            Hal::cpu().spin_loop_hint();
        }
    }
}
