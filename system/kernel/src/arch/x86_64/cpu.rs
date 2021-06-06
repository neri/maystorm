// Central Processing Unit

use crate::{
    arch::apic::*,
    io::tty::Tty,
    rt::*,
    sync::spinlock::Spinlock,
    system::{ProcessorCoreType, ProcessorIndex},
    task::scheduler::Scheduler,
    *,
};
use alloc::boxed::Box;
use bitflags::*;
use bus::pci::*;
use core::{arch::x86_64::__cpuid_count, convert::TryFrom, ffi::c_void, sync::atomic::*};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

extern "C" {
    fn asm_handle_exception(_: InterruptVector) -> usize;
    fn asm_sch_switch_context(current: *mut u8, next: *const u8);
    fn asm_sch_make_new_thread(context: *mut u8, new_sp: *mut c_void, start: usize, arg: usize);
    fn _asm_int_40() -> !;
}

static mut SHARED_CPU: SharedCpu = SharedCpu::new();

pub struct Cpu {
    pub cpu_index: ProcessorIndex,
    cpu_id: ProcessorId,
    core_type: ProcessorCoreType,
    tsc_base: u64,
    gdt: Box<GlobalDescriptorTable>,
}

#[allow(dead_code)]
struct SharedCpu {
    smt_topology: u32,
    max_cpuid_level_0: u32,
    max_cpuid_level_8: u32,
}

impl SharedCpu {
    const fn new() -> Self {
        Self {
            smt_topology: 0,
            max_cpuid_level_0: 0,
            max_cpuid_level_8: 0,
        }
    }
}

impl Cpu {
    pub(crate) unsafe fn init() {
        let pi = System::acpi_platform().processor_info.unwrap();
        System::activate_cpu(Cpu::new(ProcessorId(pi.boot_processor.local_apic_id)));

        let shared = Self::shared();
        shared.max_cpuid_level_0 = __cpuid_count(0, 0).eax;
        shared.max_cpuid_level_8 = __cpuid_count(0x8000_0000, 0).eax;

        if shared.max_cpuid_level_0 >= 0x1F {
            let cpuid1f = __cpuid_count(0x1F, 0);
            if (cpuid1f.ecx & 0xFF00) == 0x0100 {
                Self::shared().smt_topology = (1 << cpuid1f.eax) - 1;
            }
        } else if shared.max_cpuid_level_0 >= 0x0B {
            let cpuid0b = __cpuid_count(0x0B, 0);
            if (cpuid0b.ecx & 0xFF00) == 0x0100 {
                Self::shared().smt_topology = (1 << cpuid0b.eax) - 1;
            }
        }

        InterruptDescriptorTable::init();
    }

    pub(super) unsafe fn new(apic_id: ProcessorId) -> Box<Self> {
        // Currently force disabling SSE
        asm!("
            mov {0}, cr4
            btr {0}, 9
            mov cr4, {0}
            ", out(reg) _);

        let gdt = GlobalDescriptorTable::new();

        let core_type = if (apic_id.as_u32() & Self::shared().smt_topology) == 0 {
            ProcessorCoreType::Main
        } else {
            ProcessorCoreType::Sub
        };

        Box::new(Cpu {
            cpu_index: ProcessorIndex(0),
            cpu_id: apic_id,
            core_type,
            gdt,
            tsc_base: 0,
        })
    }

    #[inline]
    pub unsafe fn set_tsc_base(&mut self, value: u64) {
        self.tsc_base = value;
    }

    #[inline]
    fn shared() -> &'static mut SharedCpu {
        unsafe { &mut SHARED_CPU }
    }

    /// Returns whether or not the specified CPUID feature is supported.
    #[inline]
    pub fn has_feature(feature: Feature) -> bool {
        unsafe {
            match feature {
                Feature::F01D(bit) => {
                    (__cpuid_count(0x0000_0001, 0).edx & (1 << bit as usize)) != 0
                }
                Feature::F01C(bit) => {
                    (__cpuid_count(0x0000_0001, 0).ecx & (1 << bit as usize)) != 0
                }
                Feature::F07B(bit) => {
                    (__cpuid_count(0x0000_0007, 0).ebx & (1 << bit as usize)) != 0
                }
                Feature::F07C(bit) => {
                    (__cpuid_count(0x0000_0007, 0).ecx & (1 << bit as usize)) != 0
                }
                Feature::F07D(bit) => {
                    (__cpuid_count(0x0000_0007, 0).edx & (1 << bit as usize)) != 0
                }
                Feature::F81D(bit) => {
                    (__cpuid_count(0x8000_0001, 0).edx & (1 << bit as usize)) != 0
                }
                Feature::F81C(bit) => {
                    (__cpuid_count(0x8000_0001, 0).ecx & (1 << bit as usize)) != 0
                }
            }
        }
    }

    #[inline]
    pub fn interlocked_increment(p: &AtomicUsize) -> usize {
        p.fetch_add(1, Ordering::SeqCst)
    }

    #[inline]
    pub fn interlocked_compare_and_swap(
        p: &AtomicUsize,
        expected: usize,
        desired: usize,
    ) -> (bool, usize) {
        match p.compare_exchange(expected, desired, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(v) => (true, v),
            Err(v) => (false, v),
        }
    }

    #[inline]
    pub fn interlocked_fetch_update<F>(p: &AtomicUsize, f: F) -> Result<usize, usize>
    where
        F: FnMut(usize) -> Option<usize>,
    {
        p.fetch_update(Ordering::SeqCst, Ordering::Relaxed, f)
    }

    #[inline]
    pub fn interlocked_swap(p: &AtomicUsize, val: usize) -> usize {
        p.swap(val, Ordering::SeqCst)
    }

    #[inline]
    pub fn interlocked_test_and_set(p: &AtomicUsize, position: usize) -> bool {
        unsafe {
            let p = p as *const _ as *mut usize;
            let r: u8;
            asm!("
                lock bts [{0}], {1}
                setc {2}
                ", in(reg) p, in(reg) position, lateout(reg_byte) r);
            r != 0
        }
    }

    #[inline]
    pub fn interlocked_test_and_clear(p: &AtomicUsize, position: usize) -> bool {
        unsafe {
            let p = p as *const _ as *mut usize;
            let r: u8;
            asm!("
                lock btr [{0}], {1}
                setc {2}
                ", in(reg) p, in(reg) position, lateout(reg_byte) r);
            r != 0
        }
    }

    #[inline]
    pub(super) const fn cpu_id(&self) -> ProcessorId {
        self.cpu_id
    }

    #[inline]
    pub fn current_processor_index() -> ProcessorIndex {
        ProcessorIndex(unsafe { Self::rdtscp().1 } as usize)
    }

    #[inline]
    pub const fn processor_type(&self) -> ProcessorCoreType {
        self.core_type
    }

    #[inline]
    pub fn current_processor_type() -> ProcessorCoreType {
        let index = Self::current_processor_index();
        System::cpu(index.0).processor_type()
    }

    #[inline]
    pub(crate) unsafe fn broadcast_schedule() -> Result<(), ()> {
        match Apic::broadcast_schedule() {
            true => Ok(()),
            false => Err(()),
        }
    }

    #[inline]
    pub fn spin_loop_hint() {
        unsafe {
            asm!("pause", options(nomem, nostack));
        }
    }

    #[inline]
    pub unsafe fn halt() {
        asm!("hlt", options(nomem, nostack));
    }

    #[inline]
    pub unsafe fn enable_interrupt() {
        asm!("sti", options(nomem, nostack));
    }

    #[inline]
    pub unsafe fn disable_interrupt() {
        asm!("cli", options(nomem, nostack));
    }

    #[inline]
    pub unsafe fn stop() -> ! {
        loop {
            Self::disable_interrupt();
            Self::halt();
        }
    }

    #[inline]
    pub fn breakpoint() {
        unsafe {
            asm!("int3");
        }
    }

    pub unsafe fn reset() -> ! {
        let _ = Scheduler::freeze(true);

        Self::out8(0x0CF9, 0x06);
        asm!("out 0x92, al", in("al") 0x01 as u8);

        Cpu::stop();
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
    #[allow(dead_code)]
    pub unsafe fn register_msi(f: fn() -> ()) -> Result<(u64, u16), ()> {
        Apic::register_msi(f)
    }

    #[inline]
    pub fn secure_rand() -> Result<u64, ()> {
        if Self::has_feature(Feature::F01C(F01C::RDRND)) {
            unsafe { Self::secure_rand_unsafe().ok_or(()) }
        } else {
            Err(())
        }
    }

    /// SAFETY: Does not check the CPUID feature bit
    #[inline]
    pub unsafe fn secure_srand_unsafe() -> Option<u64> {
        let mut status: usize;
        let mut result: u64;

        asm!("
            rdseed {0}
            sbb {1}, {1}
            ", 
            out(reg) result,
            out(reg) status,
        );
        if status != 0 {
            Some(result)
        } else {
            None
        }
    }

    /// SAFETY: Does not check the CPUID feature bit
    #[inline]
    pub unsafe fn secure_rand_unsafe() -> Option<u64> {
        let mut status: usize;
        let mut result: u64;

        asm!("
            rdrand {0}
            sbb {1}, {1}
            ", 
            out(reg) result,
            out(reg) status,
        );
        if status != 0 {
            Some(result)
        } else {
            None
        }
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

    /// SAFETY: Does not check the CPUID feature bit
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

    /// SAFETY: Does not check the CPUID feature bit
    #[inline]
    pub unsafe fn read_tsc() -> u64 {
        let (tsc_raw, index) = Self::rdtscp();
        tsc_raw - System::cpu(index as usize).tsc_base
    }

    /// Launch the 32-bit legacy mode application.
    pub unsafe fn invoke_legacy(ctx: &LegacyAppContext) -> ! {
        Cpu::disable_interrupt();

        let cpu = System::cpu_mut(Cpu::current_processor_index().0);
        *cpu.gdt.item_mut(Selector::LEGACY_CODE).unwrap() = DescriptorEntry::code_legacy(
            ctx.base_of_code,
            ctx.size_of_code - 1,
            PrivilegeLevel::User,
            DefaultSize::Use32,
        );
        *cpu.gdt.item_mut(Selector::LEGACY_DATA).unwrap() = DescriptorEntry::data_legacy(
            ctx.base_of_data,
            ctx.size_of_data - 1,
            PrivilegeLevel::User,
        );
        cpu.gdt.reload();

        let rsp: u64;
        asm!("mov {0}, rsp", out(reg) rsp);
        cpu.gdt.tss.stack_pointer[0] = rsp;

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
            in (reg) Rflags::IF.bits(),
            options(noreturn));
    }

    #[inline]
    #[track_caller]
    pub unsafe fn without_interrupts<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut rax: usize;
        asm!("
            pushfq
            cli
            pop {0}
            ", lateout(reg) rax);
        let has_to_restore = Rflags::from_bits_truncate(rax).contains(Rflags::IF);

        let result = f();

        if has_to_restore {
            Cpu::enable_interrupt();
        }

        result
    }
}

#[macro_export]
macro_rules! without_interrupts {
    ( $f:expr ) => {
        Cpu::without_interrupts(|| $f)
    };
}

impl PciImpl for Cpu {
    #[inline]
    unsafe fn read_pci(&self, addr: PciConfigAddress) -> u32 {
        without_interrupts!({
            Cpu::out32(0xCF8, addr.into());
            Cpu::in32(0xCFC)
        })
    }

    #[inline]
    unsafe fn write_pci(&self, addr: PciConfigAddress, value: u32) {
        without_interrupts!({
            Cpu::out32(0xCF8, addr.into());
            Cpu::out32(0xCFC, value);
        })
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

/// Architecture-specific context data
#[repr(C)]
pub struct CpuContextData {
    _repr: [u8; Self::SIZE_OF_CONTEXT],
}

impl CpuContextData {
    pub const SIZE_OF_CONTEXT: usize = 512;
    pub const SIZE_OF_STACK: usize = 0x10000;

    #[inline]
    pub fn new() -> Self {
        Self {
            _repr: [0; Self::SIZE_OF_CONTEXT],
        }
    }

    #[inline]
    pub unsafe fn switch(&mut self, other: &Self) {
        let current = self as *const _ as *mut u8;
        let other = other as *const _ as *const u8;
        asm_sch_switch_context(current, other);
    }

    #[inline]
    pub unsafe fn init(&mut self, new_sp: *mut c_void, start: usize, arg: usize) {
        let context = self as *const _ as *mut u8;
        asm_sch_make_new_thread(context, new_sp, start, arg);
    }
}

#[repr(C, align(16))]
pub struct GlobalDescriptorTable {
    table: [DescriptorEntry; Self::NUM_ITEMS],
    tss: TaskStateSegment,
}

impl GlobalDescriptorTable {
    const NUM_ITEMS: usize = 8;

    unsafe fn new() -> Box<Self> {
        let mut gdt = Box::new(GlobalDescriptorTable {
            tss: TaskStateSegment::new(),
            table: [DescriptorEntry::null(); Self::NUM_ITEMS],
        });

        let tss_pair =
            DescriptorEntry::tss_descriptor(&gdt.tss as *const _ as usize, gdt.tss.limit());

        gdt.table[Selector::KERNEL_CODE.index()] =
            DescriptorEntry::code_segment(PrivilegeLevel::Kernel, DefaultSize::Use64);
        gdt.table[Selector::KERNEL_DATA.index()] =
            DescriptorEntry::data_segment(PrivilegeLevel::Kernel);
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
            ", out(reg) _, in(reg) Selector::KERNEL_DATA.0, in(reg) Selector::KERNEL_CODE.0);

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

/// CPUID Feature Function 0000_0001, EDX
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
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
#[derive(Debug, Copy, Clone)]
pub enum F81D {
    SYSCALL = 11,
    NX = 20,
    PDPE1GB = 26,
    RDTSCP = 27,
    LM = 29,
}

/// CPUID Feature Function 8000_0001, ECX
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
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
    pub struct Rflags: usize {
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

/// Type of x86 segment limit
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Limit(pub u16);

/// Type of x86 segment selector
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
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
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum PrivilegeLevel {
    /// Ring 0, Kernel mode
    Kernel = 0,
    /// Ring 1, Useless in 64bit mode
    Ring1,
    /// Ring 2, Useless in 64bit mode
    Ring2,
    /// Ring 3, User mode
    User,
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
            _ => PrivilegeLevel::User,
        }
    }
}

impl From<usize> for PrivilegeLevel {
    #[inline]
    fn from(value: usize) -> Self {
        Self::from_usize(value)
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq)]
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
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct InterruptVector(pub u8);

impl InterruptVector {
    pub const IPI_INVALIDATE_TLB: Self = Self(0xEE);
    pub const IPI_SCHEDULE: Self = Self(0xFC);
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, FromPrimitive)]
pub enum ExceptionType {
    DivideError = 0,
    Debug = 1,
    NonMaskable = 2,
    Breakpoint = 3,
    Overflow = 4,
    //Deprecated = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    //Deprecated = 9,
    InvalidTss = 10,
    SegmentNotPresent = 11,
    StackException = 12,
    GeneralProtection = 13,
    PageFault = 14,
    //Unavailable = 15,
    FloatingPointException = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SimdException = 19,
    Virtualization = 20,
    //Reserved
    Security = 30,
    //Reserved = 31,
    MaxReserved = 32,
}

impl ExceptionType {
    #[inline]
    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(self as u8)
    }
}

impl From<ExceptionType> for InterruptVector {
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

impl TaskStateSegment {
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
            let is_32 = (value.0 & Self::Use32 as u64) != 0;
            let is_64 = (value.0 & Self::Use64 as u64) != 0;
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
#[derive(Copy, Clone, PartialEq)]
pub struct DescriptorEntry(pub u64);

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

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    table: [DescriptorEntry; Self::MAX * 2],
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
        for vec in 0..(ExceptionType::MaxReserved as u8) {
            let vec = InterruptVector(vec);
            let offset = asm_handle_exception(vec);
            if offset != 0 {
                Self::register(vec, offset, PrivilegeLevel::Kernel);
            }
        }

        // Haribote OS Supports
        {
            let vec = InterruptVector(0x40);
            let offset = asm_handle_exception(vec);
            if offset != 0 {
                Self::register(vec, offset, PrivilegeLevel::User);
            }
        }
    }

    unsafe fn load() {
        asm!("
            push {0}
            push {1}
            lidt [rsp + 6]
            add rsp, 16
            ", in(reg) &IDT.table, in(reg) ((IDT.table.len() * 8 - 1) << 48));
    }

    #[track_caller]
    pub unsafe fn register(vec: InterruptVector, offset: usize, dpl: PrivilegeLevel) {
        let table_offset = vec.0 as usize * 2;
        if !IDT.table[table_offset].is_null() {
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
        IDT.table[table_offset + 1] = pair.high;
        IDT.table[table_offset] = pair.low;
    }
}

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Msr {
    Tsc = 0x10,
    ApicBase = 0x01b,
    MiscEnable = 0x1a0,
    TscDeadline = 0x6e0,
    Efer = 0xc000_0080,
    Star = 0xc000_0081,
    LStar = 0xc000_0082,
    CStr = 0xc000_0083,
    Fmask = 0xc000_0084,
    FsBase = 0xc000_0100,
    GsBase = 0xc000_0101,
    KernelGsBase = 0xc000_0102,
    TscAux = 0xc000_0103,
    Deadbeef = 0xdeadbeef,
}

#[repr(C)]
#[derive(Copy, Clone)]
union MsrResult {
    qword: u64,
    tuple: AccumulatorPair,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct AccumulatorPair {
    eax: u32,
    edx: u32,
}

impl Msr {
    #[inline]
    pub unsafe fn write(self, value: u64) {
        let value = MsrResult { qword: value };
        asm!("wrmsr", in("eax") value.tuple.eax, in("edx") value.tuple.edx, in("ecx") self as u32, options(nomem, nostack),);
    }

    #[inline]
    pub unsafe fn read(self) -> u64 {
        let mut eax: u32;
        let mut edx: u32;
        asm!("rdmsr", lateout("eax") eax, lateout("edx") edx, in("ecx") self as u32, options(nomem, nostack));
        MsrResult {
            tuple: AccumulatorPair { eax, edx },
        }
        .qword
    }
}

#[allow(dead_code)]
#[repr(C)]
pub(super) struct X64StackContext {
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

impl X64StackContext {
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
}

static mut GLOBAL_EXCEPTION_LOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub(super) unsafe extern "C" fn cpu_default_exception(ctx: *mut X64StackContext) {
    let is_user = GLOBAL_EXCEPTION_LOCK.synchronized(|| {
        let is_user = Scheduler::current_personality(|_| ()).is_some();
        let stdout = if is_user {
            System::stdout()
        } else {
            System::em_console() as &mut dyn Tty
        };
        let ctx = ctx.as_ref().unwrap();
        let cpu = System::current_processor();
        let cs_desc = cpu.gdt.item(ctx.cs()).unwrap();
        let ex: ExceptionType = FromPrimitive::from_u8(ctx.vector().0).unwrap();

        match cs_desc.default_operand_size().unwrap() {
            DefaultSize::Use16 | DefaultSize::Use32 => {
                let va_mask = 0xFFFF_FFFF_FFFF;
                let mask32 = 0xFFFF_FFFF;
                match ex {
                    ExceptionType::PageFault => {
                        writeln!(
                            stdout,
                            "\n#### PAGE FAULT {:04x} {:08x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ctx.error_code(),
                            ctx.cr2 & va_mask,
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
                            "\n#### {:?} err {:04x} EIP {:02x}:{:08x} ESP {:02x}:{:08x}",
                            ex,
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
                    _ => {
                        writeln!(
                            stdout,
                            "\n#### {:?} err {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
                            ex,
                            ctx.error_code(),
                            ctx.cs().0,
                            ctx.rip & va_mask,
                            ctx.ss().0,
                            ctx.rsp & va_mask,
                        )
                        .unwrap();
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

        is_user
    });

    if is_user {
        RuntimeEnvironment::exit(1);
    } else {
        loop {
            asm!("hlt");
        }
    }
}

#[inline]
#[no_mangle]
pub(super) unsafe extern "C" fn cpu_int40_handler(ctx: *mut haribote::HoeSyscallRegs) {
    let regs = ctx.as_mut().unwrap();
    Scheduler::current_personality(|personality| {
        let hoe = match personality.context() {
            PersonalityContext::Hoe(hoe) => hoe,
            _ => unreachable!(),
        };
        hoe.syscall(regs);
    });
}
