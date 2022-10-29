use super::apic::*;
use crate::{
    io::tty::Tty,
    rt::{LegacyAppContext, RuntimeEnvironment},
    system::{ProcessorCoreType, ProcessorIndex},
    task::scheduler::Scheduler,
    *,
};
use alloc::boxed::Box;
use bitflags::*;
use core::{
    arch::asm,
    arch::x86_64::__cpuid_count,
    cell::UnsafeCell,
    convert::TryFrom,
    ffi::c_void,
    mem::{size_of, transmute},
    sync::atomic::*,
};
use paste::paste;

static mut SHARED_CPU: UnsafeCell<SharedCpu> = UnsafeCell::new(SharedCpu::new());

#[allow(dead_code)]
pub struct Cpu {
    pub cpu_index: ProcessorIndex,
    apic_id: ApicId,
    core_type: ProcessorCoreType,
    tsc_base: u64,
    gdt: Box<GlobalDescriptorTable>,
}

#[allow(dead_code)]
struct SharedCpu {
    max_cpuid_level_0: u32,
    max_cpuid_level_8: u32,
    smt_topology: u32,
    has_smt: bool,
}

impl SharedCpu {
    const fn new() -> Self {
        Self {
            max_cpuid_level_0: 0,
            max_cpuid_level_8: 0,
            smt_topology: 0,
            has_smt: false,
        }
    }
}

impl Cpu {
    pub unsafe fn init() {
        let apic_id = System::acpi()
            .unwrap()
            .local_apics()
            .next()
            .map(|v| v.apic_id())
            .unwrap_or(0);
        System::activate_cpu(Cpu::new(apic_id.into()));

        let shared = Self::shared_mut();
        shared.max_cpuid_level_0 = __cpuid_count(0, 0).eax;
        shared.max_cpuid_level_8 = __cpuid_count(0x8000_0000, 0).eax;

        if shared.max_cpuid_level_0 >= 0x1F {
            let cpuid1f = __cpuid_count(0x1F, 0);
            if (cpuid1f.ecx & 0xFF00) == 0x0100 {
                shared.smt_topology = (1 << cpuid1f.eax) - 1;
            }
        } else if shared.max_cpuid_level_0 >= 0x0B {
            let cpuid0b = __cpuid_count(0x0B, 0);
            if (cpuid0b.ecx & 0xFF00) == 0x0100 {
                shared.smt_topology = (1 << cpuid0b.eax) - 1;
            }
        }

        InterruptDescriptorTable::init();
    }

    pub(super) unsafe fn new(apic_id: ApicId) -> Box<Self> {
        let gdt = GlobalDescriptorTable::new();

        let core_type = if (apic_id.as_u32() & Self::shared_mut().smt_topology) == 0 {
            ProcessorCoreType::Main
        } else {
            Self::shared_mut().has_smt = true;
            ProcessorCoreType::Sub
        };

        // if Feature::F81C(F81C::WDT).has_feature() {
        //     MSR::CPU_WATCHDOG_TIMER.write(0);
        // }

        Box::new(Cpu {
            cpu_index: ProcessorIndex(0),
            apic_id,
            core_type,
            gdt,
            tsc_base: 0,
        })
    }

    #[inline]
    pub fn set_tsc_base(&mut self, value: u64) {
        self.tsc_base = value;
    }

    #[inline]
    unsafe fn shared_mut<'a>() -> &'a mut SharedCpu {
        SHARED_CPU.get_mut()
    }

    #[inline]
    fn shared<'a>() -> &'a SharedCpu {
        unsafe { &*SHARED_CPU.get() }
    }

    #[inline]
    pub fn has_smt() -> bool {
        Self::shared().has_smt
    }

    #[inline]
    pub(super) const fn apic_id(&self) -> ApicId {
        self.apic_id
    }

    #[inline]
    pub const fn physical_id(&self) -> usize {
        self.apic_id().as_u32() as usize
    }

    #[inline]
    pub const fn processor_type(&self) -> ProcessorCoreType {
        self.core_type
    }

    #[inline]
    pub fn current_processor_type() -> ProcessorCoreType {
        let index = Hal::cpu().current_processor_index();
        System::cpu(index).processor_type()
    }

    #[inline]
    pub unsafe fn broadcast_schedule() -> Result<(), ()> {
        match Apic::broadcast_schedule() {
            true => Ok(()),
            false => Err(()),
        }
    }

    #[inline]
    pub unsafe fn out8(port: u16, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value);
    }

    #[inline]
    pub unsafe fn in8(port: u16) -> u8 {
        let mut result: u8;
        asm!("in al, dx", in("dx") port, lateout("al") result);
        result
    }

    #[inline]
    pub unsafe fn out16(port: u16, value: u16) {
        asm!("out dx, ax", in("dx") port, in("ax") value);
    }

    #[inline]
    pub unsafe fn in16(port: u16) -> u16 {
        let mut result: u16;
        asm!("in ax, dx", in("dx") port, lateout("ax") result);
        result
    }

    #[inline]
    pub unsafe fn out32(port: u16, value: u32) {
        asm!("out dx, eax", in("dx") port, in("eax") value);
    }

    #[inline]
    pub unsafe fn in32(port: u16) -> u32 {
        let mut result: u32;
        asm!("in eax, dx", in("dx") port, lateout("eax") result);
        result
    }

    #[inline]
    pub(super) fn rdtsc() -> u64 {
        let eax: u32;
        let edx: u32;
        unsafe {
            asm!("rdtsc", lateout("edx") edx, lateout("eax") eax, options(nomem, nostack));
        }
        eax as u64 + edx as u64 * 0x10000_0000
    }

    #[inline]
    pub unsafe fn rdtscp() -> (u64, u32) {
        let eax: u32;
        let edx: u32;
        let ecx: u32;
        asm!(
            "rdtscp",
            lateout("eax") eax,
            lateout("ecx") ecx,
            lateout("edx") edx,
            options(nomem, nostack),
        );
        (eax as u64 + edx as u64 * 0x10000_0000, ecx)
    }

    #[inline]
    pub unsafe fn read_tsc() -> u64 {
        let (tsc_raw, index) = Self::rdtscp();
        tsc_raw - System::cpu(ProcessorIndex(index as usize)).tsc_base
    }

    /// Launch the 32-bit legacy mode application.
    pub unsafe fn invoke_legacy(ctx: &LegacyAppContext) -> ! {
        Hal::cpu().disable_interrupt();

        let gdt = GlobalDescriptorTable::current();
        gdt.set_item(
            Selector::LEGACY_CODE,
            DescriptorEntry::code_legacy(
                ctx.base_of_code,
                ctx.size_of_code - 1,
                PrivilegeLevel::User,
                DefaultSize::Use32,
            ),
        )
        .unwrap();
        gdt.set_item(
            Selector::LEGACY_DATA,
            DescriptorEntry::data_legacy(
                ctx.base_of_data,
                ctx.size_of_data - 1,
                PrivilegeLevel::User,
            ),
        )
        .unwrap();

        let rsp: u64;
        asm!("mov {0}, rsp", out(reg) rsp);
        gdt.tss.stack_pointer[0] = rsp;

        gdt.reload();

        let rflags = Rflags::IF;

        asm!("
            mov ds, {0:e}
            mov es, {0:e}
            mov fs, {0:e}
            mov gs, {0:e}
            push {0}
            push {2}
            push {4}
            push {1}
            push {3}
            iretq
            ",
            in (reg) Selector::LEGACY_DATA.0 as usize,
            in (reg) Selector::LEGACY_CODE.0 as usize,
            in (reg) ctx.stack_pointer as usize,
            in (reg) ctx.start as usize,
            in (reg) rflags.bits(),
            options(noreturn));
    }
}

/// CPU specific context data
#[repr(C, align(64))]
pub struct CpuContextData {
    _regs: [u64; ContextIndex::Max as usize],
    _fpu: [u8; 512],
}

macro_rules! context_index {
    ($name:ident) => {
        paste! {
            pub const [<CTX_ $name>] : usize = ContextIndex::$name.to_offset();
        }
    };
    ($name:ident, $($name2:ident),+) => {
        context_index!{ $name }
        context_index!{ $($name2),+ }
    };
}

impl CpuContextData {
    pub const SIZE_OF_CONTEXT: usize = 1024;
    pub const SIZE_OF_STACK: usize = 0x10000;

    context_index! { RSP, RBP, RBX, R12, R13, R14, R15, USER_CS_DESC, USER_DS_DESC, TSS_RSP0 }
    pub const CTX_DS: usize = ContextIndex::Segs.to_offset() + 0;
    pub const CTX_ES: usize = ContextIndex::Segs.to_offset() + 2;
    pub const CTX_FS: usize = ContextIndex::Segs.to_offset() + 4;
    pub const CTX_GS: usize = ContextIndex::Segs.to_offset() + 6;
    pub const CTX_FPU: usize = ContextIndex::Max.to_offset();

    #[inline]
    pub const fn new() -> Self {
        Self {
            _regs: [0; ContextIndex::Max as usize],
            _fpu: [0; 512],
        }
    }

    pub unsafe fn switch(&mut self, other: &mut Self) {
        let gdt = GlobalDescriptorTable::current();
        Self::_switch(self, other, gdt);
    }

    #[naked]
    unsafe extern "C" fn _switch(
        current: *mut Self,
        other: *const Self,
        gdt: *mut GlobalDescriptorTable,
    ) {
        asm!(
            "
            mov [rdi + {CTX_RSP}], rsp
            mov [rdi + {CTX_RBP}], rbp
            mov [rdi + {CTX_RBX}], rbx
            mov [rdi + {CTX_R12}], r12
            mov [rdi + {CTX_R13}], r13
            mov [rdi + {CTX_R14}], r14
            mov [rdi + {CTX_R15}], r15
            mov [rdi + {CTX_DS}], ds
            mov [rdi + {CTX_ES}], es
            mov [rdi + {CTX_FS}], fs
            mov [rdi + {CTX_GS}], gs
            fxsave [rdi + {CTX_FPU}]

            mov rax, [rsi + {CTX_USER_CS}]
            xchg rax, [rdx + {USER_CS_IDX} * 8]
            mov [rdi + {CTX_USER_CS}], rax
        
            mov rax, [rsi + {CTX_USER_DS}]
            xchg rax, [rdx + {USER_DS_IDX} * 8]
            mov [rdi + {CTX_USER_DS}], rax

            mov rax, [rsi + {CTX_TSS_RSP0}]
            xchg rax, [rdx + {OFFSET_TSS} + {TSS_OFF_RSP0}]
            mov [rdi + {CTX_TSS_RSP0}], rax

            fxrstor [rsi + {CTX_FPU}]
            mov rsp, [rsi + {CTX_RSP}]
            mov rbp, [rsi + {CTX_RBP}]
            mov rbx, [rsi + {CTX_RBX}]
            mov r12, [rsi + {CTX_R12}]
            mov r13, [rsi + {CTX_R13}]
            mov r14, [rsi + {CTX_R14}]
            mov r15, [rsi + {CTX_R15}]
            mov ds, [rsi + {CTX_DS}]
            mov es, [rsi + {CTX_ES}]
            mov fs, [rsi + {CTX_FS}]
            mov gs, [rsi + {CTX_GS}]

            xor eax, eax
            xor ecx, ecx
            xor edx, edx
            xor esi, esi
            xor edi, edi
            xor r8, r8
            xor r9, r9
            xor r10, r10
            xor r11, r11
            ret
            ",
            CTX_RSP = const Self::CTX_RSP,
            CTX_RBP = const Self::CTX_RBP,
            CTX_RBX = const Self::CTX_RBX,
            CTX_R12 = const Self::CTX_R12,
            CTX_R13 = const Self::CTX_R13,
            CTX_R14 = const Self::CTX_R14,
            CTX_R15 = const Self::CTX_R15,
            CTX_FPU = const Self::CTX_FPU,
            CTX_TSS_RSP0 = const Self::CTX_TSS_RSP0,
            OFFSET_TSS = const GlobalDescriptorTable::OFFSET_TSS,
            TSS_OFF_RSP0 = const TaskStateSegment::OFFSET_RSP0,
            CTX_DS = const Self::CTX_DS,
            CTX_ES = const Self::CTX_ES,
            CTX_FS = const Self::CTX_FS,
            CTX_GS = const Self::CTX_GS,
            CTX_USER_CS = const Self::CTX_USER_CS_DESC,
            CTX_USER_DS = const Self::CTX_USER_DS_DESC,
            USER_CS_IDX = const Selector::LEGACY_CODE.index(),
            USER_DS_IDX = const Selector::LEGACY_DATA.index(),
            options(noreturn)
        );
    }

    #[inline]
    pub unsafe fn init(&mut self, new_sp: *mut c_void, start: usize, arg: usize) {
        asm!("
            sub {new_sp}, 0x18
            mov [{new_sp}], {new_thread}
            mov [{new_sp} + 0x08], {start}
            mov [{new_sp} + 0x10], {arg}
            mov [{0} + {CTX_RSP}], {new_sp}
            xor {temp:e}, {temp:e}
            mov [{0} + {CTX_USER_CS}], {temp}
            mov [{0} + {CTX_USER_DS}], {temp}
            ",
            in(reg) self,
            new_sp = in(reg) new_sp,
            start = in(reg) start,
            arg = in(reg) arg,
            new_thread = in(reg) Self::_new_thread,
            temp = out(reg) _,
            CTX_RSP = const Self::CTX_RSP,
            CTX_USER_CS = const Self::CTX_USER_CS_DESC,
            CTX_USER_DS = const Self::CTX_USER_DS_DESC,
        );
    }

    #[naked]
    unsafe extern "C" fn _new_thread() {
        asm!(
            "
            fninit
            mov eax, 0x00001F80
            push rax
            ldmxcsr [rsp]
            pop rax
            pxor xmm0, xmm0
            pxor xmm1, xmm1
            pxor xmm2, xmm2
            pxor xmm3, xmm3
            pxor xmm4, xmm4
            pxor xmm5, xmm5
            pxor xmm6, xmm6
            pxor xmm7, xmm7
            pxor xmm8, xmm8
            pxor xmm9, xmm9
            pxor xmm10, xmm10
            pxor xmm11, xmm11
            pxor xmm12, xmm12
            pxor xmm13, xmm13
            pxor xmm14, xmm14
            pxor xmm15, xmm15

            call {setup_new_thread}

            sti
            pop rax
            pop rdi
            call rax
            ",
            setup_new_thread = sym task::scheduler::sch_setup_new_thread,
            options(noreturn)
        );
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ContextIndex {
    USER_CS_DESC = 2,
    USER_DS_DESC,
    RSP,
    RBP,
    RBX,
    R12,
    R13,
    R14,
    R15,
    TSS_RSP0,
    Segs,
    Max = 32,
}

impl ContextIndex {
    #[inline]
    pub const fn to_offset(&self) -> usize {
        size_of::<usize>() * (*self as usize)
    }
}

#[repr(C, align(16))]
pub struct GlobalDescriptorTable {
    table: [DescriptorEntry; Self::NUM_ITEMS],
    tss: TaskStateSegment,
}

impl !Send for GlobalDescriptorTable {}

impl GlobalDescriptorTable {
    pub const NUM_ITEMS: usize = 8;
    pub const OFFSET_TSS: usize = 8 * Self::NUM_ITEMS;

    unsafe fn new() -> Box<Self> {
        let mut gdt = Box::new(GlobalDescriptorTable {
            tss: TaskStateSegment::new(),
            table: [DescriptorEntry::null(); Self::NUM_ITEMS],
        });

        gdt.set_item(
            Selector::KERNEL_CODE,
            DescriptorEntry::code_segment(PrivilegeLevel::Kernel, DefaultSize::Use64),
        )
        .unwrap();
        gdt.set_item(
            Selector::KERNEL_DATA,
            DescriptorEntry::data_segment(PrivilegeLevel::Kernel),
        )
        .unwrap();

        let tss_pair =
            DescriptorEntry::tss_descriptor(&gdt.tss as *const _ as usize, gdt.tss.limit());
        let tss_index = Selector::SYSTEM_TSS.index();
        gdt.table[tss_index] = tss_pair.low;
        gdt.table[tss_index + 1] = tss_pair.high;

        gdt.reload();
        asm!("
            mov {0}, rsp
            push {1:r}
            push {0}
            pushfq
            push {2:r}
            .byte 0xE8, 2, 0, 0, 0, 0xEB, 0x02, 0x48, 0xCF
            mov ds, {1:e}
            mov es, {1:e}
            mov fs, {1:e}
            mov gs, {1:e}
            ", out(reg) _, in(reg) Selector::KERNEL_DATA.0, in(reg) Selector::KERNEL_CODE.0
        );

        asm!("ltr {0:x}", in(reg) Selector::SYSTEM_TSS.0);

        gdt
    }

    #[inline]
    pub unsafe fn item(&self, selector: Selector) -> Option<&DescriptorEntry> {
        let index = selector.index();
        self.table.get(index)
    }

    #[inline]
    pub unsafe fn item_mut(&mut self, selector: Selector) -> Option<&mut DescriptorEntry> {
        let index = selector.index();
        self.table.get_mut(index)
    }

    #[inline]
    pub unsafe fn set_item(&mut self, selector: Selector, desc: DescriptorEntry) -> Option<()> {
        let index = selector.index();
        self.table.get_mut(index).map(|v| *v = desc)
    }

    #[inline]
    pub unsafe fn current<'a>() -> &'a mut GlobalDescriptorTable {
        let gdt: usize;
        asm!("
            sub rsp, 16
            sgdt [rsp + 6]
            mov {0}, [rsp + 8]
            add rsp, 16
            ", out(reg) gdt
        );
        &mut *(gdt as *mut GlobalDescriptorTable)
    }

    /// Reload GDT
    unsafe fn reload(&self) {
        asm!("
            push {0}
            push {1}
            lgdt [rsp + 6]
            add rsp, 16
            ", in(reg) &self.table, in(reg) ((self.table.len() * 8 - 1) << 48));
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Feature {
    F01D(F01D),
    F01C(F01C),
    F07B(F070B),
    F07C(F070C),
    F07D(F070D),
    F81D(F81D),
    F81C(F81C),
}

impl Feature {
    pub unsafe fn has_feature(&self) -> bool {
        match *self {
            Self::F01D(bit) => (__cpuid_count(0x0000_0001, 0).edx & (1 << bit as usize)) != 0,
            Self::F01C(bit) => (__cpuid_count(0x0000_0001, 0).ecx & (1 << bit as usize)) != 0,
            Self::F07B(bit) => (__cpuid_count(0x0000_0007, 0).ebx & (1 << bit as usize)) != 0,
            Self::F07C(bit) => (__cpuid_count(0x0000_0007, 0).ecx & (1 << bit as usize)) != 0,
            Self::F07D(bit) => (__cpuid_count(0x0000_0007, 0).edx & (1 << bit as usize)) != 0,
            Self::F81D(bit) => (__cpuid_count(0x8000_0001, 0).edx & (1 << bit as usize)) != 0,
            Self::F81C(bit) => (__cpuid_count(0x8000_0001, 0).ecx & (1 << bit as usize)) != 0,
        }
    }
}

/// CPUID Feature Function 0000_0001, EDX
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F01D {
    FPU = 0,
    VME = 1,
    DE = 2,
    PSE = 3,
    TSC = 4,
    MSR = 5,
    PAE = 6,
    MCE = 7,
    CX8 = 8,
    APIC = 9,
    SEP = 11,
    MTRR = 12,
    MGE = 13,
    MCA = 14,
    CMOV = 15,
    PAT = 16,
    PSE36 = 17,
    PSN = 18,
    CLFSH = 19,
    DS = 21,
    ACPI = 22,
    MMX = 23,
    FXSR = 24,
    SSE = 25,
    SSE2 = 26,
    SS = 27,
    HTT = 28,
    TM = 29,
    IA64 = 30,
    PBE = 31,
}

/// CPUID Feature Function 0000_0001, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F01C {
    SSE3 = 0,
    PCLMULQDQ = 1,
    DTES64 = 2,
    MONITOR = 3,
    DS_CPL = 4,
    VMX = 5,
    SMX = 6,
    EST = 7,
    TM2 = 8,
    SSSE3 = 9,
    CNXT_ID = 10,
    SDBG = 11,
    FMA = 12,
    CX16 = 13,
    XTPR = 14,
    PDCM = 15,
    PCID = 17,
    DCA = 18,
    SSE4_1 = 19,
    SSE4_2 = 20,
    X2APIC = 21,
    MOVBE = 22,
    POPCNT = 23,
    TSC_DEADLINE = 24,
    AES = 25,
    XSAVE = 26,
    OSXSAVE = 27,
    AVX = 28,
    F16C = 29,
    RDRND = 30,
    HYPERVISOR = 31,
}

/// CPUID Feature Function 0000_0007, EBX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F070B {
    FSGSBASE = 0,
    IA32_TSC_ADJUST = 1,
    SGX = 2,
    BMI1 = 3,
    HLE = 4,
    AVX2 = 5,
    FDP_EXCPTN_ONLY = 6,
    SMEP = 7,
    BMI2 = 8,
    ERMS = 9,
    INVPCID = 10,
    RTM = 11,
    PQM = 12,
    // FPU CS and FPU DS deprecated = 13,
    MPX = 14,
    PQE = 15,
    AVX512_F = 16,
    AVX512_DQ = 17,
    RDSEED = 18,
    ADX = 19,
    SMAP = 20,
    AVX512_IFMA = 21,
    PCOMMIT = 22,
    CLFLUSHIPT = 23,
    CLWB = 24,
    INTEL_PT = 25,
    AVX512_PF = 26,
    AVX512_ER = 27,
    AVX512_CD = 28,
    SHA = 29,
    AVX512_BW = 30,
    AVX512_VL = 31,
}

/// CPUID Feature Function 0000_0007, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F070C {
    PREFETCHWT1 = 0,
    AVX512_VBMI = 1,
    UMIP = 2,
    PKU = 3,
    OSPKE = 4,
    WAITPKG = 5,
    AVX512_VBMI2 = 6,
    CET_SS = 7,
    GFNI = 8,
    VAES = 9,
    VPCLMULQDQ = 10,
    AVX512_VNNI = 11,
    AVX512_BITALG = 12,
    AVX512_VPOPCNTDQ = 14,
    LA57 = 16,
    RDPID = 22,
    CLDEMOTE = 25,
    MOVDIRI = 27,
    MOVDIR64B = 28,
    ENQCMD = 29,
    SGX_LC = 30,
    PKS = 31,
}

/// CPUID Feature Function 0000_0007, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F070D {
    AVX512_4VNNIW = 2,
    AVX512_4FMAPS = 3,
    FSRM = 4,
    AVX512_VP2INTERSECT = 8,
    SRBDS_CTRL = 9,
    MD_CLEAR = 10,
    TSX_FORCE_ABORT = 13,
    SERIALIZE = 14,
    HYBRID = 15,
    TSXLDTRK = 16,
    PCONFIG = 18,
    LBR = 19,
    CET_IBT = 20,
    AMX_BF16 = 22,
    AMX_TILE = 24,
    AMX_INT8 = 25,
    IBRS_IBPB = 26,
    STIBP = 27,
    L1D_FLUSH = 28,
    IA32_ARCH_CAPABILITIES = 29,
    IA32_CORE_CAPABILITIES = 30,
    SSBD = 31,
}

/// CPUID Feature Function 8000_0001, EDX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F81D {
    SYSCALL = 11,
    NX = 20,
    PDPE1GB = 26,
    RDTSCP = 27,
    LM = 29,
}

/// CPUID Feature Function 8000_0001, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum F81C {
    LAHF_LM = 0,
    CMP_LEGACY = 1,
    SVM = 2,
    EXTAPIC = 3,
    CR8_LEGACY = 4,
    ABM = 5,
    SSE4A = 6,
    MISALIGNSSE = 7,
    _3DNOWPREFETCH = 8,
    OSVW = 9,
    IBS = 10,
    XOP = 11,
    SKINIT = 12,
    WDT = 13,
    LWP = 15,
    FMA4 = 16,
    TCE = 17,
    NODEID_MSR = 19,
    TBM = 21,
    TOPOEXT = 22,
    PERFCTR_CORE = 23,
    PERFCTR_NB = 24,
    DBX = 26,
    PERFTSC = 27,
    PCX_L2I = 28,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct Rflags: u64 {
        const CF    = 0x0000_0001;
        const PF    = 0x0000_0004;
        const AF    = 0x0000_0010;
        const ZF    = 0x0000_0040;
        const SF    = 0x0000_0080;
        const TF    = 0x0000_0100;
        const IF    = 0x0000_0200;
        const DF    = 0x0000_0400;
        const OF    = 0x0000_0800;
        const IOPL3 = 0x0000_3000;
        const NT    = 0x0000_4000;
        const RF    = 0x0001_0000;
        const VM    = 0x0002_0000;
        const AC    = 0x0004_0000;
        const VIF   = 0x0008_0000;
        const VIP   = 0x0010_0000;
        const ID    = 0x0020_0000;
    }
}

impl Rflags {
    #[inline]
    pub const fn iopl(&self) -> PrivilegeLevel {
        PrivilegeLevel::from_usize((self.bits() & Self::IOPL3.bits()) as usize >> 12)
    }

    #[inline]
    pub const fn set_iopl(&mut self, iopl: PrivilegeLevel) {
        *self = Self::from_bits_retain((self.bits() & !Self::IOPL3.bits()) | ((iopl as u64) << 12));
    }
}

/// Type of x86 segment limit
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Limit(pub u16);

/// Type of x86 segment selector
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selector(pub u16);

impl Selector {
    /// The NULL selector that does not contain anything
    pub const NULL: Selector = Selector(0);
    pub const KERNEL_CODE: Selector = Selector::new(1, PrivilegeLevel::Kernel);
    pub const KERNEL_DATA: Selector = Selector::new(2, PrivilegeLevel::Kernel);
    // pub const USER_CODE: Selector = Selector::new(3, PrivilegeLevel::User);
    pub const LEGACY_CODE: Selector = Selector::new(4, PrivilegeLevel::User);
    pub const LEGACY_DATA: Selector = Selector::new(5, PrivilegeLevel::User);
    pub const SYSTEM_TSS: Selector = Selector::new(6, PrivilegeLevel::Kernel);

    /// Make a new instance of the selector from the specified index and RPL
    #[inline]
    pub const fn new(index: usize, rpl: PrivilegeLevel) -> Self {
        Selector((index << 3) as u16 | rpl as u16)
    }

    /// Returns the requested privilege level in the selector
    #[inline]
    pub const fn rpl(self) -> PrivilegeLevel {
        PrivilegeLevel::from_usize(self.0 as usize)
    }

    /// Returns the index field in the selector
    #[inline]
    pub const fn index(self) -> usize {
        (self.0 >> 3) as usize
    }
}

/// DPL, CPL, RPL and IOPL
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrivilegeLevel {
    /// Ring 0, Kernel mode
    Kernel = 0,
    /// Ring 1, Useless in 64bit mode
    Ring1 = 1,
    /// Ring 2, Useless in 64bit mode
    Ring2 = 2,
    /// Ring 3, User mode
    User = 3,
}

impl PrivilegeLevel {
    #[inline]
    pub const fn as_descriptor_entry(self) -> u64 {
        (self as u64) << 45
    }

    #[inline]
    pub const fn from_usize(value: usize) -> Self {
        match value & 3 {
            0 => PrivilegeLevel::Kernel,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::User,
            _ => unreachable!(),
        }
    }
}

impl const From<usize> for PrivilegeLevel {
    #[inline]
    fn from(value: usize) -> Self {
        Self::from_usize(value)
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorType {
    Null = 0,
    Tss = 9,
    TssBusy = 11,
    InterruptGate = 14,
    TrapGate = 15,
}

impl DescriptorType {
    #[inline]
    pub const fn as_descriptor_entry(self) -> u64 {
        let ty = self as u64;
        ty << 40
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterruptVector(pub u8);

impl InterruptVector {
    pub const IPI_INVALIDATE_TLB: Self = Self(0xEE);
    pub const IPI_SCHEDULE: Self = Self(0xFC);
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExceptionType {
    /// #DE
    DivideError = 0,
    /// #DB
    Debug = 1,
    /// NMI
    NonMaskable = 2,
    /// #BP
    Breakpoint = 3,
    /// #OF
    Overflow = 4,
    //Deprecated = 5,
    /// #UD
    InvalidOpcode = 6,
    /// #NM
    DeviceNotAvailable = 7,
    /// #DF
    DoubleFault = 8,
    //Deprecated = 9,
    /// #TS
    InvalidTss = 10,
    /// #NP
    SegmentNotPresent = 11,
    /// #SS
    StackException = 12,
    /// #GP
    GeneralProtection = 13,
    /// #PF
    PageFault = 14,
    //Unavailable = 15,
    /// #MF
    FloatingPointException = 16,
    /// #AC
    AlignmentCheck = 17,
    /// #MC
    MachineCheck = 18,
    /// #XM
    SimdException = 19,
    /// #CE
    Virtualization = 20,
    //Reserved
    /// #SX
    Security = 30,
    //Reserved = 31,
    MAX = 32,
}

impl ExceptionType {
    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(self as u8)
    }

    #[inline]
    pub const unsafe fn from_vec(vec: InterruptVector) -> Self {
        transmute(vec.0)
    }

    #[inline]
    pub const fn has_error_code(&self) -> bool {
        match self {
            ExceptionType::DoubleFault
            | ExceptionType::InvalidTss
            | ExceptionType::SegmentNotPresent
            | ExceptionType::StackException
            | ExceptionType::GeneralProtection
            | ExceptionType::PageFault
            | ExceptionType::AlignmentCheck
            | ExceptionType::Security => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn mnemonic(&self) -> &'static str {
        match self {
            ExceptionType::DivideError => "#DE",
            ExceptionType::Debug => "#DB",
            ExceptionType::NonMaskable => "NMI",
            ExceptionType::Breakpoint => "#BP",
            ExceptionType::Overflow => "#OV",
            ExceptionType::InvalidOpcode => "#UD",
            ExceptionType::DeviceNotAvailable => "#NM",
            ExceptionType::DoubleFault => "#DF",
            ExceptionType::InvalidTss => "#TS",
            ExceptionType::SegmentNotPresent => "#NP",
            ExceptionType::StackException => "#SS",
            ExceptionType::GeneralProtection => "#GP",
            ExceptionType::PageFault => "#PF",
            ExceptionType::FloatingPointException => "#MF",
            ExceptionType::AlignmentCheck => "#AC",
            ExceptionType::MachineCheck => "#MC",
            ExceptionType::SimdException => "#XM",
            ExceptionType::Virtualization => "#VE",
            ExceptionType::Security => "#SX",
            _ => "",
        }
    }
}

impl const From<ExceptionType> for InterruptVector {
    #[inline]
    fn from(ex: ExceptionType) -> Self {
        InterruptVector(ex as u8)
    }
}

#[repr(C, packed)]
#[derive(Default)]
struct TaskStateSegment {
    _reserved_1: u32,
    stack_pointer: [u64; 3],
    _reserved_2: [u32; 2],
    ist: [u64; 7],
    _reserved_3: [u32; 2],
    iomap_base: u16,
}

impl !Send for TaskStateSegment {}

impl TaskStateSegment {
    pub const OFFSET_RSP0: usize = 0x04;

    #[inline]
    const fn new() -> Self {
        Self {
            _reserved_1: 0,
            stack_pointer: [0; 3],
            _reserved_2: [0, 0],
            ist: [0; 7],
            _reserved_3: [0, 0],
            iomap_base: 0,
        }
    }

    #[inline]
    const fn limit(&self) -> Limit {
        Limit(0x67)
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DefaultSize {
    Use16 = 0x0000_0000_0000_0000,
    Use32 = 0x0040_0000_0000_0000,
    Use64 = 0x0020_0000_0000_0000,
}

impl DefaultSize {
    #[inline]
    pub const fn as_descriptor_entry(self) -> u64 {
        self as u64
    }

    #[inline]
    pub const fn from_descriptor(value: DescriptorEntry) -> Option<Self> {
        if value.is_code_segment() {
            let is_32 = (value.0 & Self::Use32.as_descriptor_entry()) != 0;
            let is_64 = (value.0 & Self::Use64.as_descriptor_entry()) != 0;
            match (is_32, is_64) {
                (false, false) => Some(Self::Use16),
                (false, true) => Some(Self::Use64),
                (true, false) => Some(Self::Use32),
                (true, true) => None,
            }
        } else {
            None
        }
    }
}

impl TryFrom<DescriptorEntry> for DefaultSize {
    type Error = ();
    fn try_from(value: DescriptorEntry) -> Result<Self, Self::Error> {
        Self::from_descriptor(value).ok_or(())
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct DescriptorEntry(u64);

impl DescriptorEntry {
    pub const PRESENT: u64 = 0x8000_0000_0000;
    pub const GRANULARITY: u64 = 0x0080_0000_0000_0000;
    pub const BIG_DATA: u64 = 0x0040_0000_0000_0000;

    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn code_segment(dpl: PrivilegeLevel, size: DefaultSize) -> DescriptorEntry {
        DescriptorEntry(
            0x000F_1A00_0000_FFFFu64
                | Self::PRESENT
                | Self::GRANULARITY
                | dpl.as_descriptor_entry()
                | size.as_descriptor_entry(),
        )
    }

    #[inline]
    pub const fn data_segment(dpl: PrivilegeLevel) -> DescriptorEntry {
        DescriptorEntry(
            0x000F_1200_0000_FFFFu64
                | Self::PRESENT
                | Self::GRANULARITY
                | Self::BIG_DATA
                | dpl.as_descriptor_entry(),
        )
    }

    #[inline]
    pub const fn code_legacy(
        base: u32,
        limit: u32,
        dpl: PrivilegeLevel,
        size: DefaultSize,
    ) -> DescriptorEntry {
        let limit = if limit > 0xFFFF {
            Self::GRANULARITY
                | ((limit as u64) >> 10) & 0xFFFF
                | ((limit as u64 & 0xF000_0000) << 16)
        } else {
            limit as u64
        };
        DescriptorEntry(
            0x0000_1A00_0000_0000u64
                | limit
                | Self::PRESENT
                | dpl.as_descriptor_entry()
                | size.as_descriptor_entry()
                | ((base as u64 & 0x00FF_FFFF) << 16)
                | ((base as u64 & 0xFF00_0000) << 32),
        )
    }

    #[inline]
    pub const fn data_legacy(base: u32, limit: u32, dpl: PrivilegeLevel) -> DescriptorEntry {
        let limit = if limit > 0xFFFF {
            Self::GRANULARITY | ((limit as u64) >> 10) & 0xFFFF | (limit as u64 & 0xF000_0000) << 16
        } else {
            limit as u64
        };
        DescriptorEntry(
            0x0000_1200_0000_0000u64
                | limit
                | Self::PRESENT
                | Self::BIG_DATA
                | dpl.as_descriptor_entry()
                | ((base as u64 & 0x00FF_FFFF) << 16)
                | ((base as u64 & 0xFF00_0000) << 32),
        )
    }

    #[inline]
    pub const fn tss_descriptor(offset: usize, limit: Limit) -> DescriptorPair {
        let offset = offset as u64;
        let low = DescriptorEntry(
            limit.0 as u64
                | Self::PRESENT
                | DescriptorType::Tss.as_descriptor_entry()
                | ((offset & 0x00FF_FFFF) << 16)
                | ((offset & 0xFF00_0000) << 32),
        );
        let high = DescriptorEntry(offset >> 32);
        DescriptorPair::new(low, high)
    }

    #[inline]
    pub const fn gate_descriptor(
        offset: usize,
        sel: Selector,
        dpl: PrivilegeLevel,
        ty: DescriptorType,
    ) -> DescriptorPair {
        let offset = offset as u64;
        let low = DescriptorEntry(
            (offset & 0xFFFF)
                | (sel.0 as u64) << 16
                | Self::PRESENT
                | dpl.as_descriptor_entry()
                | ty.as_descriptor_entry()
                | (offset & 0xFFFF_0000) << 32,
        );
        let high = DescriptorEntry(offset >> 32);

        DescriptorPair::new(low, high)
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_present(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }

    #[inline]
    pub const fn is_segment(&self) -> bool {
        (self.0 & 0x1000_0000_0000) != 0
    }

    #[inline]
    pub const fn is_code_segment(&self) -> bool {
        self.is_segment() && (self.0 & 0x0800_0000_0000) != 0
    }

    #[inline]
    pub const fn default_operand_size(&self) -> Option<DefaultSize> {
        DefaultSize::from_descriptor(*self)
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub struct DescriptorPair {
    pub low: DescriptorEntry,
    pub high: DescriptorEntry,
}

impl DescriptorPair {
    #[inline]
    pub const fn new(low: DescriptorEntry, high: DescriptorEntry) -> Self {
        DescriptorPair { low, high }
    }
}

static mut IDT: UnsafeCell<InterruptDescriptorTable> =
    UnsafeCell::new(InterruptDescriptorTable::new());

#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    table: [DescriptorEntry; Self::MAX * 2],
}

impl !Send for InterruptDescriptorTable {}

macro_rules! register_exception {
    ($mnemonic:ident) => {
        paste! {
            Self::register(
                ExceptionType::$mnemonic.as_vec(),
                [<exc_ $mnemonic>] as usize,
                PrivilegeLevel::Kernel,
            );
        }
    };
}

impl InterruptDescriptorTable {
    const MAX: usize = 256;

    const fn new() -> Self {
        InterruptDescriptorTable {
            table: [DescriptorEntry::null(); Self::MAX * 2],
        }
    }

    unsafe fn init() {
        Self::load();

        register_exception!(DivideError);
        register_exception!(Breakpoint);
        register_exception!(InvalidOpcode);
        register_exception!(DeviceNotAvailable);
        register_exception!(DoubleFault);
        register_exception!(GeneralProtection);
        register_exception!(PageFault);
        register_exception!(MachineCheck);
        register_exception!(SimdException);

        {
            // Haribote OS Supports
            let vec = InterruptVector(0x40);
            Self::register(vec, cpu_int40_handler as usize, PrivilegeLevel::User);
        }
    }

    unsafe fn load() {
        let idt = &*IDT.get();
        asm!("
            push {0}
            push {1}
            lidt [rsp + 6]
            add rsp, 16
            ", in(reg) &(idt.table), in(reg) ((idt.table.len() * 8 - 1) << 48));
    }

    #[track_caller]
    pub unsafe fn register(vec: InterruptVector, offset: usize, dpl: PrivilegeLevel) {
        let table_offset = vec.0 as usize * 2;
        let mut idt = IDT.get_mut();
        if !idt.table[table_offset].is_null() {
            panic!("IDT entry #{} is already in use", vec.0);
        }
        let pair = DescriptorEntry::gate_descriptor(
            offset,
            Selector::KERNEL_CODE,
            dpl,
            if dpl == PrivilegeLevel::Kernel {
                DescriptorType::InterruptGate
            } else {
                DescriptorType::TrapGate
            },
        );
        idt.table[table_offset + 1] = pair.high;
        idt.table[table_offset] = pair.low;
        fence(Ordering::SeqCst);
    }
}

#[repr(u32)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MSR {
    TSC = 0x0000_0010,
    APIC_BASE = 0x0000_001B,
    MISC_ENABLE = 0x0000_01A0,
    TSC_DEADLINE = 0x0000_06E0,
    EFER = 0xC000_0080,
    STAR = 0xC000_0081,
    LSTAR = 0xC000_0082,
    CSTAR = 0xC000_0083,
    FMASK = 0xC000_0084,
    FS_BASE = 0xC000_0100,
    GS_BASE = 0xC000_0101,
    KERNEL_GS_BASE = 0xC000_0102,
    TSC_AUX = 0xC000_0103,
    CPU_WATCHDOG_TIMER = 0xC001_0074,
}

#[repr(C)]
#[derive(Copy, Clone)]
union MsrResult {
    qword: u64,
    pair: EaxAndEdx,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct EaxAndEdx {
    eax: u32,
    edx: u32,
}

impl MSR {
    #[inline]
    pub unsafe fn write(self, value: u64) {
        let value = MsrResult { qword: value };
        asm!(
            "wrmsr",
            in("eax") value.pair.eax,
            in("edx") value.pair.edx,
            in("ecx") self as u32,
        );
    }

    #[inline]
    pub unsafe fn read(self) -> u64 {
        let eax: u32;
        let edx: u32;
        asm!(
            "rdmsr",
            lateout("eax") eax,
            lateout("edx") edx,
            in("ecx") self as u32,
        );
        MsrResult {
            pair: EaxAndEdx { eax, edx },
        }
        .qword
    }
}

#[allow(dead_code)]
#[repr(C)]
pub(super) struct X64ExceptionContext {
    _mxcsr: u64,
    cr2: u64,
    _gs: u64,
    _fs: u64,
    _es: u64,
    _ds: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    _vector: u64,
    _error_code: u64,
    rip: u64,
    _cs: u64,
    rflags: Rflags,
    rsp: u64,
    _ss: u64,
}

impl X64ExceptionContext {
    #[inline]
    pub const fn cs(&self) -> Selector {
        Selector(self._cs as u16)
    }

    #[inline]
    pub const fn ds(&self) -> Selector {
        Selector(self._ds as u16)
    }

    #[inline]
    pub const fn es(&self) -> Selector {
        Selector(self._es as u16)
    }

    #[inline]
    pub const fn fs(&self) -> Selector {
        Selector(self._fs as u16)
    }

    #[inline]
    pub const fn gs(&self) -> Selector {
        Selector(self._gs as u16)
    }

    #[inline]
    pub const fn ss(&self) -> Selector {
        Selector(self._ss as u16)
    }

    #[inline]
    pub const fn error_code(&self) -> u16 {
        self._error_code as u16
    }

    #[inline]
    pub const fn vector(&self) -> InterruptVector {
        InterruptVector(self._vector as u8)
    }

    #[inline]
    pub const fn mxcsr(&self) -> u32 {
        self._mxcsr as u32
    }
}

static GLOBAL_EXCEPTION_LOCK: Spinlock = Spinlock::new();

unsafe extern "C" fn handle_default_exception(ctx: &X64ExceptionContext) {
    let is_user = GLOBAL_EXCEPTION_LOCK.synchronized(|| {
        let is_user = Scheduler::current_personality().is_some();
        let stdout = if is_user {
            System::stdout()
        } else {
            System::em_console() as &mut dyn Tty
        };
        stdout.set_attribute(0x0F);

        let cs_desc = GlobalDescriptorTable::current().item(ctx.cs()).unwrap();
        let ex = ExceptionType::from_vec(ctx.vector());

        match cs_desc.default_operand_size().unwrap() {
            DefaultSize::Use16 | DefaultSize::Use32 => {
                let mask32 = u32::MAX as u64;
                match ex {
                    ExceptionType::PageFault => {
                        writeln!(
                            stdout,
                            "\n#### PAGE FAULT {:04x} {:08x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ctx.error_code(),
                            ctx.cr2 & mask32,
                            ctx.cs().0,
                            ctx.rip & mask32,
                            ctx.ss().0,
                            ctx.rsp & mask32,
                        )
                        .unwrap();
                    }
                    _ => {
                        writeln!(
                            stdout,
                            "\n#### EXCEPTION {} err {:04x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ex.mnemonic(),
                            ctx.error_code(),
                            ctx.cs().0,
                            ctx.rip & mask32,
                            ctx.ss().0,
                            ctx.rsp & mask32,
                        )
                        .unwrap();
                    }
                }

                println!(
                    "EAX {:08x} EBX {:08x} ECX {:08x} EDX {:08x} EFLAGS {:08x}",
                    ctx.rax & mask32,
                    ctx.rbx & mask32,
                    ctx.rcx & mask32,
                    ctx.rdx & mask32,
                    ctx.rflags.bits(),
                );
                println!(
                    "EBP {:08x} ESI {:08x} EDI {:08x} DS {:04x} ES {:04x} FS {:04x} GS {:04x}",
                    ctx.rbp & mask32,
                    ctx.rsi & mask32,
                    ctx.rdi & mask32,
                    ctx.ds().0,
                    ctx.es().0,
                    ctx.fs().0,
                    ctx.gs().0,
                );
            }
            DefaultSize::Use64 => {
                let va_mask = 0xFFFF_FFFF_FFFF;
                match ex {
                    ExceptionType::PageFault => {
                        writeln!(
                        stdout,
                        "\n#### PAGE FAULT {:04x} {:012x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                        ctx.error_code(),
                        ctx.cr2 & va_mask,
                        ctx.cs().0,
                        ctx.rip & va_mask,
                        ctx.ss().0,
                        ctx.rsp & va_mask,
                    )
                        .unwrap();
                    }
                    ExceptionType::SimdException => {
                        writeln!(
                            stdout,
                            "\n#### SIMD EXCEPTION {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                            ctx.mxcsr(),
                            ctx.cs().0,
                            ctx.rip & va_mask,
                            ctx.ss().0,
                            ctx.rsp & va_mask,
                        )
                            .unwrap();
                        }
                    _ => {
                        if ex.has_error_code() {
                            writeln!(
                                stdout,
                                "\n#### EXCEPTION {} err {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                ex.mnemonic(),
                                ctx.error_code(),
                                ctx.cs().0,
                                ctx.rip & va_mask,
                                ctx.ss().0,
                                ctx.rsp & va_mask,
                            )
                            .unwrap();
                        } else {
                            writeln!(
                                stdout,
                                "\n#### EXCEPTION {} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                                ex.mnemonic(),
                                ctx.cs().0,
                                ctx.rip & va_mask,
                                ctx.ss().0,
                                ctx.rsp & va_mask,
                            )
                            .unwrap();
                        }
                    }
                }

                writeln!(
                    stdout,
                    "rax {:016x} rsi {:016x} r11 {:016x} fl {:08x}
rbx {:016x} rdi {:016x} r12 {:016x} ds {:04x}
rcx {:016x} r8  {:016x} r13 {:016x} es {:04x}
rdx {:016x} r9  {:016x} r14 {:016x} fs {:04x}
rbp {:016x} r10 {:016x} r15 {:016x} gs {:04x}",
                    ctx.rax,
                    ctx.rsi,
                    ctx.r11,
                    ctx.rflags.bits(),
                    ctx.rbx,
                    ctx.rdi,
                    ctx.r12,
                    ctx.ds().0,
                    ctx.rcx,
                    ctx.r8,
                    ctx.r13,
                    ctx.es().0,
                    ctx.rdx,
                    ctx.r9,
                    ctx.r14,
                    ctx.fs().0,
                    ctx.rbp,
                    ctx.r10,
                    ctx.r15,
                    ctx.gs().0,
                )
                .unwrap();
            }
        }

        stdout.set_attribute(0x00);
        is_user
    });

    if is_user {
        RuntimeEnvironment::exit(1);
    } else {
        panic!("Unhandled Exception in kernel mode");
    }
}

macro_rules! exception_handler {
    ($mnemonic:ident, $handler:ident) => {
        paste! {
            #[naked]
            #[allow(non_snake_case)]
            unsafe extern "C" fn [<exc_ $mnemonic>]() {
                asm!("
                push ${exno}
                push rax
                push rcx
                push rdx
                push rbx
                push rbp
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15
                mov eax, ds
                push rax
                mov ecx, es
                push rcx
                push fs
                push gs
                mov rax, cr2
                push rax
                xor eax, eax
                push rax
                stmxcsr [rsp]
                mov rbp, rsp
                and rsp, 0xfffffffffffffff0
                cld
            
                mov rdi, rbp
                call {handler}

                lea rsp, [rbp + 8 * 6]
                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rbp
                pop rbx
                pop rdx
                pop rcx
                pop rax
                add rsp, 16
                iretq
                ",
                exno = const ExceptionType::$mnemonic.as_vec().0 as usize,
                handler = sym $handler,
                options(noreturn));
            }
        }
    };
}

macro_rules! exception_handler_noerr {
    ($mnemonic:ident, $handler:ident) => {
        paste! {
            #[naked]
            #[allow(non_snake_case)]
            unsafe extern "C" fn [<exc_ $mnemonic>]() {
                asm!("
                push 0
                push ${exno}
                push rax
                push rcx
                push rdx
                push rbx
                push rbp
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15
                mov eax, ds
                push rax
                mov ecx, es
                push rcx
                push fs
                push gs
                mov rax, cr2
                push rax
                xor eax, eax
                push rax
                stmxcsr [rsp]
                mov rbp, rsp
                and rsp, 0xfffffffffffffff0
                cld
            
                mov rdi, rbp
                call {handler}

                lea rsp, [rbp + 8 * 6]
                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rbp
                pop rbx
                pop rdx
                pop rcx
                pop rax
                add rsp, 16
                iretq
                ",
                exno = const ExceptionType::$mnemonic.as_vec().0 as usize,
                handler = sym $handler,
                options(noreturn));
            }
        }
    };
}

exception_handler_noerr!(DivideError, handle_default_exception);
exception_handler_noerr!(Breakpoint, handle_default_exception);
exception_handler_noerr!(InvalidOpcode, handle_default_exception);
exception_handler_noerr!(DeviceNotAvailable, handle_default_exception);
exception_handler!(DoubleFault, handle_default_exception);
exception_handler!(GeneralProtection, handle_default_exception);
exception_handler!(PageFault, handle_default_exception);
exception_handler_noerr!(SimdException, handle_default_exception);
exception_handler_noerr!(MachineCheck, handle_default_exception);

/// Haribote OS System call Emulation
#[naked]
unsafe extern "C" fn cpu_int40_handler() {
    asm!(
        "
    push rbp
    sub rsp, 24
    mov rbp, rsp
    mov [rbp], eax
    mov [rbp + 4], ecx
    mov [rbp + 8], edx
    mov [rbp + 12], ebx
    mov [rbp + 16], esi
    mov [rbp + 20], edi
    mov eax, [rbp + 32]
    mov [rbp + 28], eax
    and rsp, 0xfffffffffffffff0
    cld

    mov rdi, rbp
    call hoe_syscall

    mov eax, [rbp]
    mov ecx, [rbp + 4]
    mov edx, [rbp + 8]
    mov ebx, [rbp + 12]
    mov esi, [rbp + 16]
    mov edi, [rbp + 20]
    mov r8d, [rbp + 24]
    lea rsp, [rbp + 8 * 4]
    mov ebp, r8d
    iretq
    ",
        options(noreturn)
    );
}

#[repr(C)]
pub struct LegacySyscallContext {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub eip: u32,
}
