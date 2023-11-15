//! Kernel Invocation for x86-64

use super::*;
use core::{
    arch::{asm, global_asm, x86_64::__cpuid},
    ffi::c_void,
};

pub struct Invocation();

extern "C" {
    fn _invoke_kernel_stub() -> !;
    fn _end_invoke_kernel_stub() -> !;
}

global_asm!(
    "
.global _invoke_kernel_stub
.global _end_invoke_kernel_stub
_invoke_kernel_stub:
    mov cr3, r8
    .byte 0xEB, 0x00
    lea rsp, [r10 - 0x20]
    call r9
    ud2
_end_invoke_kernel_stub:
"
);

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
        unsafe {
            // For Intel processors, unlock NXE disable. (Surface 3)
            if self.is_intel_processor() {
                asm!("
                    rdmsr
                    btr edx, 2
                    wrmsr
                    ",in("ecx") Self::IA32_MISC_ENABLE_MSR,
                    out("eax")_,
                    out("edx") _,
                );
            }

            // Enable NXE
            asm!("
                rdmsr
                bts eax, 11
                wrmsr
                ", in("ecx") Self::IA32_EFER_MSR,
                out("eax") _,
                out("edx") _,
            );

            // Jump to Trampoline Code to avoid problems over 4GB
            let base = _invoke_kernel_stub as usize;
            let end = _end_invoke_kernel_stub as usize;
            let count = end - base;
            let kernel_stub = (0x0800usize) as *mut c_void;
            kernel_stub.copy_from_nonoverlapping(base as *const _, count);

            asm!(
                "jmp rax",
                in("rax") kernel_stub,
                in("rcx") info,
                in("rdx") 0,
                in("rsi") 0,
                in("rdi") info,
                in("r8") info.master_cr3,
                in("r9") entry.0,
                in("r10") new_sp.0,
                options(noreturn)
            );
        }
    }
}
