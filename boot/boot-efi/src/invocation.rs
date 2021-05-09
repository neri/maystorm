// Kernel Invocation

use crate::page::*;
use bootprot::*;
// #[cfg(any(target_arch = "x86_64"))]
// use core::arch::x86_64::__cpuid_count;

pub struct Invocation {}

impl Invocation {
    const IA32_EFER_MSR: u32 = 0xC000_0080;

    /// Invoke kernel
    pub unsafe fn invoke_kernel(
        info: &BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
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
