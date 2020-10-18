// Central Processing Unit

use crate::arch::apic::Apic;
use crate::system::*;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;
use bus::pci::*;
use core::fmt;

#[allow(dead_code)]
pub struct Cpu {
    pub cpu_index: ProcessorIndex,
    gdt: Box<GlobalDescriptorTable>,
}

extern "C" {
    fn _asm_int_00() -> !;
    fn _asm_int_03() -> !;
    fn _asm_int_06() -> !;
    fn _asm_int_08() -> !;
    fn _asm_int_0D() -> !;
    fn _asm_int_0E() -> !;
}

impl Cpu {
    pub(crate) unsafe fn init() {
        InterruptDescriptorTable::init();
    }

    pub(crate) unsafe fn new() -> Box<Self> {
        let gdt = GlobalDescriptorTable::new();
        let cpu = Box::new(Cpu {
            cpu_index: ProcessorIndex(0),
            gdt,
        });

        // Currently force disabling SSE
        asm!("
            mov {0}, cr4
            btr {0}, 9
            mov cr4, {0}
            ", out(reg) _);

        cpu
    }

    #[inline]
    pub fn current_processor_id() -> ProcessorId {
        Apic::current_processor_id()
    }

    #[inline]
    pub fn current_processor_index() -> Option<ProcessorIndex> {
        Apic::current_processor_index()
    }

    #[inline]
    pub fn spin_loop_hint() {
        unsafe {
            asm!("pause");
        }
    }

    #[inline]
    pub unsafe fn halt() {
        asm!("hlt");
    }

    #[inline]
    pub unsafe fn enable_interrupt() {
        asm!("sti");
    }

    #[inline]
    pub unsafe fn disable_interrupt() {
        asm!("cli");
    }

    #[inline]
    pub(crate) unsafe fn stop() -> ! {
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

    pub(crate) unsafe fn reset() -> ! {
        let _ = MyScheduler::freeze(true);
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
    #[track_caller]
    pub(crate) fn assert_without_interrupt() {
        let flags = unsafe {
            let mut rax: usize;
            asm!("
                pushfq
                pop {0}
                ", lateout(reg) rax);
            Rflags::from_bits_unchecked(rax)
        };
        assert!(!flags.contains(Rflags::IF));
    }

    #[inline]
    pub(crate) unsafe fn without_interrupts<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut rax: usize;
        asm!("
            pushfq
            cli
            pop {0}
            ", lateout(reg) rax);
        let flags = Rflags::from_bits_unchecked(rax);

        let result = f();

        if flags.contains(Rflags::IF) {
            Self::enable_interrupt();
        }

        result
    }

    const PCI_CONFIG_ENABLED: u32 = 0x8000_0000;

    #[inline]
    pub(crate) unsafe fn read_pci(addr: PciConfigAddress) -> u32 {
        Cpu::without_interrupts(|| {
            Cpu::out32(0xCF8, addr.as_u32() | Self::PCI_CONFIG_ENABLED);
            Cpu::in32(0xCFC)
        })
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) unsafe fn write_pci(addr: PciConfigAddress, value: u32) {
        Cpu::without_interrupts(|| {
            Cpu::out32(0xCF8, addr.as_u32() | Self::PCI_CONFIG_ENABLED);
            Cpu::out32(0xCFC, value);
        })
    }
}

#[repr(C, align(16))]
pub struct GlobalDescriptorTable {
    table: [DescriptorEntry; Self::NUM_ITEMS],
    tss: TaskStateSegment,
}

impl GlobalDescriptorTable {
    const NUM_ITEMS: usize = 8;

    pub fn new() -> Box<Self> {
        let mut gdt = Box::new(GlobalDescriptorTable {
            table: [DescriptorEntry::null(); Self::NUM_ITEMS],
            tss: TaskStateSegment::new(),
        });

        let tss_pair = DescriptorEntry::tss_descriptor(
            VirtualAddress(&gdt.tss as *const _ as usize),
            gdt.tss.limit(),
        );

        gdt.table[Selector::KERNEL_CODE.index()] =
            DescriptorEntry::code_segment(PrivilegeLevel::Kernel, DefaultSize::Use64);
        gdt.table[Selector::KERNEL_DATA.index()] =
            DescriptorEntry::data_segment(PrivilegeLevel::Kernel);
        let tss_index = Selector::SYSTEM_TSS.index();
        gdt.table[tss_index] = tss_pair.low;
        gdt.table[tss_index + 1] = tss_pair.high;

        unsafe {
            gdt.reload();
        }
        gdt
    }

    // Reload GDT and Segment Selectors
    unsafe fn reload(&self) {
        asm!("
            push {0}
            push {1}
            lgdt [rsp + 6]
            add rsp, 16
            ", in(reg) &self.table, in(reg) ((self.table.len() * 8 - 1) << 48));
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
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct CpuidRegs {
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
}

#[derive(Copy, Clone)]
pub union Cpuid {
    regs: CpuidRegs,
    pub bytes: [u8; 16],
}

impl Cpuid {
    #[inline]
    pub fn perform(&mut self) {
        unsafe {
            asm!("cpuid",
                inlateout("eax") self.regs.eax,
                inlateout("ecx") self.regs.ecx,
                lateout("edx") self.regs.edx,
                lateout("ebx") self.regs.ebx,
            );
        }
    }

    #[inline]
    pub fn cpuid(eax: u32, ecx: u32) -> Self {
        let mut p = Self {
            regs: CpuidRegs {
                eax,
                ecx,
                ..CpuidRegs::default()
            },
        };
        p.perform();
        p
    }

    #[inline]
    pub fn eax(&self) -> u32 {
        unsafe { self.regs.eax }
    }

    #[inline]
    pub fn ecx(&self) -> u32 {
        unsafe { self.regs.ecx }
    }

    #[inline]
    pub fn edx(&self) -> u32 {
        unsafe { self.regs.edx }
    }

    #[inline]
    pub fn ebx(&self) -> u32 {
        unsafe { self.regs.ebx }
    }
}

impl Default for Cpuid {
    fn default() -> Self {
        Self {
            regs: CpuidRegs::default(),
        }
    }
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

impl fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VirtAddr({:#016x})", self.0)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Limit(pub u16);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Selector(pub u16);

impl Selector {
    pub const NULL: Selector = Selector(0);
    pub const KERNEL_CODE: Selector = Selector::new(1, PrivilegeLevel::Kernel);
    pub const KERNEL_DATA: Selector = Selector::new(2, PrivilegeLevel::Kernel);
    pub const SYSTEM_TSS: Selector = Selector::new(6, PrivilegeLevel::Kernel);

    #[inline]
    pub const fn new(index: usize, rpl: PrivilegeLevel) -> Self {
        Selector((index << 3) as u16 | rpl as u16)
    }

    #[inline]
    pub const fn rpl(self) -> PrivilegeLevel {
        PrivilegeLevel::from_usize(self.0 as usize)
    }

    #[inline]
    pub const fn index(self) -> usize {
        (self.0 >> 3) as usize
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum PrivilegeLevel {
    Kernel = 0,
    System1,
    System2,
    User,
}

impl PrivilegeLevel {
    pub const fn as_descriptor_entry(self) -> u64 {
        let dpl = self as u64;
        dpl << 13
    }

    pub const fn from_usize(value: usize) -> Self {
        match value & 3 {
            0 => PrivilegeLevel::Kernel,
            1 => PrivilegeLevel::System1,
            2 => PrivilegeLevel::System2,
            _ => PrivilegeLevel::User,
        }
    }
}

impl From<usize> for PrivilegeLevel {
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
    pub const fn as_descriptor_entry(self) -> u64 {
        let ty = self as u64;
        ty << 40
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct InterruptVector(pub u8);

// impl core::ops::Add<u8> for InterruptVector {
//     type Output = Self;
//     fn add(self, rhs: u8) -> Self {
//         Self(self.0 + rhs)
//     }
// }

// impl core::ops::Sub<u8> for InterruptVector {
//     type Output = Self;
//     fn sub(self, rhs: u8) -> Self {
//         Self(self.0 - rhs)
//     }
// }

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Exception {
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
}

impl Exception {
    pub const fn as_vec(self) -> InterruptVector {
        InterruptVector(self as u8)
    }
}

impl From<Exception> for InterruptVector {
    fn from(ex: Exception) -> Self {
        InterruptVector(ex as u8)
    }
}

#[repr(C, packed)]
#[derive(Default)]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub stack_pointer: [u64; 3],
    reserved_2: u32,
    pub ist: [u64; 7],
    reserved_3: u64,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved_1: 0,
            stack_pointer: [0; 3],
            reserved_2: 0,
            ist: [0; 7],
            reserved_3: 0,
            iomap_base: 0,
        }
    }

    #[inline]
    pub const fn limit(&self) -> Limit {
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
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq)]
pub struct DescriptorEntry(pub u64);

impl DescriptorEntry {
    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn present() -> u64 {
        0x8000_0000_0000
    }

    #[inline]
    pub const fn granularity() -> u64 {
        0x0080_0000_0000_0000
    }

    #[inline]
    pub const fn big_data() -> u64 {
        0x0040_0000_0000_0000
    }

    #[inline]
    pub const fn code_segment(dpl: PrivilegeLevel, size: DefaultSize) -> DescriptorEntry {
        DescriptorEntry(
            0x000F_1A00_0000_FFFFu64
                | Self::present()
                | Self::granularity()
                | dpl.as_descriptor_entry()
                | size.as_descriptor_entry(),
        )
    }

    #[inline]
    pub const fn data_segment(dpl: PrivilegeLevel) -> DescriptorEntry {
        DescriptorEntry(
            0x000F_1200_0000_FFFFu64
                | Self::present()
                | Self::granularity()
                | Self::big_data()
                | dpl.as_descriptor_entry(),
        )
    }

    #[inline]
    pub const fn tss_descriptor(offset: VirtualAddress, limit: Limit) -> DescriptorPair {
        let offset = offset.0 as u64;
        let low = DescriptorEntry(
            limit.0 as u64
                | Self::present()
                | DescriptorType::Tss.as_descriptor_entry()
                | (offset & 0x00FF_FFFF) << 16
                | (offset & 0xFF00_0000) << 32,
        );
        let high = DescriptorEntry(offset >> 32);
        DescriptorPair::new(low, high)
    }

    #[inline]
    pub const fn gate_descriptor(
        offset: VirtualAddress,
        sel: Selector,
        dpl: PrivilegeLevel,
        ty: DescriptorType,
    ) -> DescriptorPair {
        let offset = offset.0 as u64;
        let low = DescriptorEntry(
            (offset & 0xFFFF)
                | (sel.0 as u64) << 16
                | Self::present()
                | dpl.as_descriptor_entry()
                | ty.as_descriptor_entry()
                | (offset & 0xFFFF_0000) << 32,
        );
        let high = DescriptorEntry(offset >> 32);

        DescriptorPair::new(low, high)
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
        Self::register(
            Exception::DivideError.into(),
            VirtualAddress(_asm_int_00 as usize),
        );
        Self::register(
            Exception::Breakpoint.into(),
            VirtualAddress(_asm_int_03 as usize),
        );
        Self::register(
            Exception::InvalidOpcode.into(),
            VirtualAddress(_asm_int_06 as usize),
        );
        Self::register(
            Exception::DoubleFault.into(),
            VirtualAddress(_asm_int_08 as usize),
        );
        Self::register(
            Exception::GeneralProtection.into(),
            VirtualAddress(_asm_int_0D as usize),
        );
        Self::register(
            Exception::PageFault.into(),
            VirtualAddress(_asm_int_0E as usize),
        );
    }

    unsafe fn load() {
        asm!("
            push {0}
            push {1}
            lidt [rsp+6]
            add rsp, 16
            ", in(reg) &IDT.table, in(reg) ((IDT.table.len() * 8 - 1) << 48));
    }

    pub unsafe fn register(vec: InterruptVector, offset: VirtualAddress) {
        let pair = DescriptorEntry::gate_descriptor(
            offset,
            Selector::KERNEL_CODE,
            PrivilegeLevel::Kernel,
            DescriptorType::InterruptGate,
        );
        let table_offset = vec.0 as usize * 2;
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
pub union MsrResult {
    pub qword: u64,
    pub tuple: EaxEdx,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct EaxEdx {
    pub eax: u32,
    pub edx: u32,
}

impl Msr {
    #[inline]
    pub unsafe fn write(self, value: u64) {
        let value = MsrResult { qword: value };
        asm!("wrmsr", in("eax") value.tuple.eax, in("edx") value.tuple.edx, in("ecx") self as u32);
    }

    #[inline]
    pub unsafe fn read(self) -> u64 {
        let mut eax: u32;
        let mut edx: u32;
        asm!("rdmsr", lateout("eax") eax, lateout("edx") edx, in("ecx") self as u32);
        MsrResult {
            tuple: EaxEdx { eax: eax, edx: edx },
        }
        .qword
    }
}

#[repr(C, packed)]
pub struct X64StackContext {
    pub cr2: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub vector: InterruptVector,
    _padding_1: [u8; 7],
    pub error_code: u16,
    _padding_2: [u16; 3],
    pub rip: u64,
    pub cs: u16,
    _padding_3: [u16; 3],
    pub rflags: Rflags,
    pub rsp: u64,
    pub ss: u16,
    _padding_4: [u16; 3],
}

static mut GLOBAL_EXCEPTION_LOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub unsafe extern "C" fn cpu_default_exception(ctx: *mut X64StackContext) {
    GLOBAL_EXCEPTION_LOCK.lock();
    let ctx = ctx.as_ref().unwrap();
    stdout().set_cursor_enabled(false);
    let va_mask = 0xFFFF_FFFF_FFFF;
    if Exception::PageFault.as_vec() == ctx.vector {
        println!(
            "\n#### EXCEPTION {:02x} err {:04x} {:012x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
            ctx.vector.0,
            ctx.error_code,
            ctx.cr2 & va_mask,
            ctx.cs,
            ctx.rip & va_mask,
            ctx.ss,
            ctx.rsp & va_mask,
        );
    } else {
        println!(
            "\n#### EXCEPTION {:02x} err {:04x} rip {:02x}:{:012x} rsp {:02x}:{:012x}",
            ctx.vector.0,
            ctx.error_code,
            ctx.cs,
            ctx.rip & va_mask,
            ctx.ss,
            ctx.rsp & va_mask,
        );
    }

    println!(
        "rax {:016x} rsi {:016x} r11 {:016x} rfl {:08x}
rbx {:016x} rdi {:016x} r12 {:016x}
rcx {:016x} r8  {:016x} r13 {:016x}
rdx {:016x} r9  {:016x} r14 {:016x}
rbp {:016x} r10 {:016x} r15 {:016x}",
        ctx.rax,
        ctx.rsi,
        ctx.r11,
        ctx.rflags.bits(),
        ctx.rbx,
        ctx.rdi,
        ctx.r12,
        ctx.rcx,
        ctx.r8,
        ctx.r13,
        ctx.rdx,
        ctx.r9,
        ctx.r14,
        ctx.rbp,
        ctx.r10,
        ctx.r15,
    );

    GLOBAL_EXCEPTION_LOCK.unlock();
    loop {
        asm!("hlt");
    }
}
