//! Kernel Invocation for x86-64

use super::*;
use core::arch::{asm, x86_64::__cpuid};

pub struct Invocation();

impl Invocation {
    const IA32_EFER_MSR: u32 = 0xC000_0080;
    const IA32_MISC_ENABLE_MSR: u32 = 0x0000_01A0;

    #[inline]
    pub const fn new() -> Self {
        Self()
    }

    #[inline]
    fn is_intel_processor(&self) -> bool {
        let cpuid = unsafe { __cpuid(0) };
        // GenuineIntel
        cpuid.ebx == 0x756e6547 && cpuid.edx == 0x49656e69 && cpuid.ecx == 0x6c65746e
    }
}

impl Invoke for Invocation {
    fn is_compatible(&self) -> bool {
        let cpuid = unsafe { __cpuid(0x8000_0001) };
        // RDTSCP
        if cpuid.edx & (1 << 27) == 0 {
            return false;
        }
        return true;
    }

    unsafe fn invoke_kernel(
        &self,
        info: &BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
        // For Intel processors, unlock NXE disable. (Surface 3)
        if self.is_intel_processor() {
            asm!("
                rdmsr
                btr edx, 2
                wrmsr
                ",in("ecx") Self::IA32_MISC_ENABLE_MSR, out("eax")_, out("edx") _);
        }

        // Enable NXE
        asm!("
            rdmsr
            bts eax, 11
            wrmsr
            ", in("ecx") Self::IA32_EFER_MSR, out("eax") _, out("edx") _,);

        // Sets up a new CR3
        asm!("
            mov cr3, {0}
            .byte 0xEB, 0x00
            ", in(reg) info.master_cr3);

        asm!("
            lea rsp, [{1} - 0x20]
            call {0}
            ud2
            ",
            in(reg) entry.0,
            in(reg) new_sp.0,
            in("rcx") info,
            in("rdx") 0,
            in("rsi") 0,
            in("rdi") info,
            options(noreturn)
        );
    }
}
