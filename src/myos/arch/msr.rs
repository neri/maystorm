// Model-specific register

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Msr {
    Ia32Tsc = 0x10,
    Ia32ApicBase = 0x01B,
    Deadbeef = 0xdeadbeef,
}

#[repr(C)]
#[derive(Copy, Clone)]
union MsrResult {
    pub qword: u64,
    pub tuple: EaxEdx,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct EaxEdx {
    pub eax: u32,
    pub edx: u32,
}

impl Msr {
    pub unsafe fn write(&self, value: u64) {
        let value = MsrResult { qword: value };
        llvm_asm!("wrmsr"
        :: "{eax}"(value.tuple.eax),"{edx}"(value.tuple.edx),"{ecx}"(*self));
    }

    pub unsafe fn read(&self) -> u64 {
        let mut eax: u32;
        let mut edx: u32;
        llvm_asm!("rdmsr"
        : "={eax}"(eax),"={edx}"(edx)
        : "{ecx}"(*self));
        MsrResult {
            tuple: EaxEdx { eax: eax, edx: edx },
        }
        .qword
    }
}
