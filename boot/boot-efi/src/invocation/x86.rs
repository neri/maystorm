//! Kernel Invocation for 32-bit UEFI running on x86-64 processors

use super::*;
use core::arch::{asm, x86::__cpuid};

pub struct Invocation;

impl Invocation {
    const IA32_EFER_MSR: u32 = 0xC000_0080;
    const IA32_MISC_ENABLE_MSR: u32 = 0x0000_01A0;

    #[inline]
    pub const fn new() -> Self {
        Self
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
        let cpuid = unsafe { __cpuid(0x8000_0000) };
        if cpuid.eax < 0x8000_0001 {
            return false;
        }
        let cpuid = unsafe { __cpuid(0x8000_0001) };
        // LM
        if cpuid.edx & (1 << 29) == 0 {
            return false;
        }
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
            // Disable paging before entering Long Mode.
            asm!("
                mov {0}, cr0
                btr {0}, 31
                mov cr0, {0}
                ", out(reg) _);

            // For Intel processors, unlock NXE disable. (ex: Surface 3)
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

            // Set up a GDT for Long Mode
            GDT.fix_up();
            asm!("lgdt [{0}]", in(reg) &GDT);

            // Set up a CR3 for Long Mode
            asm!("mov cr3, {0}", in(reg) info.master_cr3 as usize);

            // Enable NXE & LME
            asm!("
                rdmsr
                bts eax, 8
                bts eax, 11
                wrmsr
                ", in("ecx") Self::IA32_EFER_MSR, out("eax") _, out("edx") _,);

            // Enable PAE
            asm!("
                mov {0}, cr4
                bts {0}, 5
                mov cr4, {0}
                ", out(reg) _);

            // Enter to Long Mode
            asm!("
                mov {0}, cr0
                bts {0}, 31
                mov cr0, {0}
                ", out(reg) _);

            let params = [entry.0, new_sp.0 - 0x20];

            // Trampoline code to jump to 64-bit kernel
            asm!("
                                                // [bits 32]
                .byte 0x6a, 0x08                //      push 0x08
                .byte 0xe8, 0x08, 0, 0, 0       //      call _jmpf
                                                // [bits 64]
                .byte 0x48, 0x8b, 0x60, 0x08    //      mov rsp, [rax + 8]
                .byte 0xff, 0x10                //      call [rax + 0]
                .byte 0x0f, 0x0b                //      ud2
                                                // [bits 32]
                                                // _jmpf:
                .byte 0xff, 0x2c, 0x24          //      jmp far [esp]
                ",
                in("eax") &params,
                in("edi") info,
                options(noreturn),
            );
        }
    }
}

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

#[repr(C, align(16))]
struct GlobalDescriptorTable {
    table: [u16; 12],
}

impl GlobalDescriptorTable {
    #[inline]
    const fn new() -> Self {
        Self {
            table: [
                0xFFFF, 0, 0, 0, // 00 NULL
                0xFFFF, 0x0000, 0x9A00, 0x00AF, // 08 DPL0 CODE64 FLAT
                0xFFFF, 0x0000, 0x9200, 0x00CF, // 10 DPL0 DATA FLAT
            ],
        }
    }

    fn fix_up(&mut self) {
        let base = &self.table as *const _ as usize;
        self.table[1] = (base & 0xFFFF) as u16;
        self.table[2] = ((base >> 16) & 0xFFFF) as u16;
    }
}
