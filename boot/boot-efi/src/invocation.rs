// Kernel Invocation

use crate::page::*;
use bootprot::*;
use core::arch::asm;

pub struct Invocation {}

impl Invocation {
    const IA32_EFER_MSR: u32 = 0xC000_0080;
    const IA32_MISC_ENABLE_MSR: u32 = 0x0000_01A0;

    /// Invoke kernel
    pub unsafe fn invoke_kernel(
        info: &BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
        let cpuid = core::arch::x86_64::__cpuid(0);
        // GenuineIntel
        if cpuid.ebx == 0x756e6547 && cpuid.edx == 0x49656e69 && cpuid.ecx == 0x6c65746e {
            // If Intel, then unlock NXE disable
            asm!("
                rdmsr
                btr edx, 2
                wrmsr
                ",in("ecx") Self::IA32_MISC_ENABLE_MSR, lateout("eax")_, lateout("edx") _);
        }

        // Enable NXE
        asm!("
            rdmsr
            bts eax, 11
            wrmsr
            ", in("ecx") Self::IA32_EFER_MSR, lateout("eax") _, lateout("edx") _,);

        // Set new CR3
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
