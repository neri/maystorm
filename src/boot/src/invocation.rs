// Kernel Invocation

use crate::page::*;
use bootinfo::*;

pub struct Invocation {}

impl Invocation {
    pub unsafe fn invoke_kernel(
        info: BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> ! {
        let mut info = info;
        PageManager::finalize(&mut info);
        // Set new CR3
        asm!("
        mov cr3, {0}
        .byte 0xEB, 0x00
        ", in(reg) info.master_cr3);

        // Invoke kernel
        asm!("
        xor edx, edx
        xor esi, esi
        mov rdi, rcx
        mov rsp, r8
        xor r8, r8
        xor r9, r9
        call rax
        ",
            in("rax") entry.0,
            in("rcx") &info,
            in("r8") new_sp.0,
        );
        loop {}
    }
}
