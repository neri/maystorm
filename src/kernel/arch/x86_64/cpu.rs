// Central Processing Unit

use crate::kernel::arch::apic::Apic;
use crate::kernel::system::*;
use crate::*;
use alloc::boxed::Box;
use bitflags::*;

// #[derive(Debug)]
pub struct Cpu {
    pub cpu_id: ProcessorId,
    pub gdt: Box<GlobalDescriptorTable>,
    pub tss: Box<TaskStateSegment>,
}

extern "C" {
    fn _int00();
    fn _int03();
    fn _int06();
    fn _int08();
    fn _int0D();
    fn _int0E();
}

impl Cpu {
    pub(crate) unsafe fn new(cpuid: ProcessorId) -> Box<Self> {
        let tss = TaskStateSegment::new();
        let gdt = GlobalDescriptorTable::new(&tss);
        let cpu = Box::new(Cpu {
            cpu_id: cpuid,
            gdt: gdt,
            tss: tss,
        });

        // Currently force disabling SSE
        asm!("
        mov {0}, cr4
        btr {0}, 9
        mov cr4, {0}
        ", out(reg) _);

        cpu
    }

    pub(crate) unsafe fn init() {
        InterruptDescriptorTable::init();

        if let acpi::InterruptModel::Apic(apic) =
            System::shared().acpi().interrupt_model.as_ref().unwrap()
        {
            Apic::init(apic);
        } else {
            panic!("NO APIC");
        }
    }

    pub fn current_processor_id() -> ProcessorId {
        Apic::current_processor_id()
    }

    pub fn current_processor_index() -> Option<ProcessorIndex> {
        Apic::current_processor_index()
    }

    pub fn relax() {
        unsafe {
            asm!("pause");
        }
    }

    pub unsafe fn halt() {
        asm!("hlt");
    }

    pub fn breakpoint() {
        unsafe {
            asm!("int3");
        }
    }

    pub unsafe fn reset() -> ! {
        // io_out8(0x0CF9, 0x06);
        Cpu::out8(0x0092, 0x01);
        loop {
            Cpu::halt()
        }
    }

    pub unsafe fn out8(port: u16, value: u8) {
        asm!("out dx, al", in("dx") port, in("al") value);
    }

    #[must_use]
    pub(crate) fn assert_without_interrupt() -> bool {
        let flags = unsafe {
            let mut rax: usize;
            asm!("
            pushfq
            pop {0}
            ", lateout(reg) rax);
            Eflags::from_bits_unchecked(rax)
        };
        !flags.contains(Eflags::IF)
    }

    #[inline]
    pub(crate) fn without_interrupts<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let flags = unsafe {
            let mut rax: usize;
            asm!("
            pushfq
            cli
            pop {0}
            ", lateout(reg) rax);
            Eflags::from_bits_unchecked(rax)
        };

        let r = f();

        if flags.contains(Eflags::IF) {
            unsafe {
                asm!("sti");
            }
        }

        r
    }

    pub(crate) unsafe fn stop() -> ! {
        loop {
            asm!("cli");
            Self::halt();
        }
    }
}

bitflags! {
    pub struct Eflags: usize {
        const CF = 0x00000001;
        const PF = 0x00000004;
        const AF = 0x00000010;
        const ZF = 0x00000040;
        const SF = 0x00000080;
        const TF = 0x00000100;
        const IF = 0x00000200;
        const DF = 0x00000400;
        const OF = 0x00000800;
        // const IOPLMASK = 0x00003000;
        // const IOPL3 = IOPLMASK;
        const NT = 0x00004000;
        const RF = 0x00010000;
        const VM = 0x00020000;
        const AC = 0x00040000;
        const VIF = 0x00080000;
        const VIP = 0x00100000;
        const ID = 0x00200000;
    }
}

const MAX_GDT: usize = 8;
const MAX_IDT: usize = 256;
const KERNEL_CODE: Selector = Selector::new(1, PrivilegeLevel::Kernel);
const KERNEL_DATA: Selector = Selector::new(2, PrivilegeLevel::Kernel);
const TSS: Selector = Selector::new(6, PrivilegeLevel::Kernel);

use core::fmt;
impl fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VirtualAddress({:#016x})", self.0)
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

    pub const fn new(index: usize, rpl: PrivilegeLevel) -> Self {
        Selector((index << 3) as u16 | rpl as u16)
    }

    pub fn rpl(self) -> PrivilegeLevel {
        PrivilegeLevel::from(self.0 as usize)
    }

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
}

impl From<usize> for PrivilegeLevel {
    fn from(value: usize) -> PrivilegeLevel {
        match value & 3 {
            0 => PrivilegeLevel::Kernel,
            1 => PrivilegeLevel::System1,
            2 => PrivilegeLevel::System2,
            3 => PrivilegeLevel::User,
            _ => unreachable!(),
        }
    }
}

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
pub struct TaskStateSegment {
    reserved_1: u32,
    pub stack_pointer: [u64; 3],
    reserved_2: u32,
    pub ist: [u64; 7],
    reserved_3: u64,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub fn new() -> Box<Self> {
        Box::new(TaskStateSegment {
            stack_pointer: [0; 3],
            ist: [0; 7],
            iomap_base: 0,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
        })
    }

    pub fn limit(&self) -> Limit {
        Limit(0x67)
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DefaultSize {
    Use16 = 0x0000_0000_0000_0000,
    Use32 = 0x00C0_0000_0000_0000,
    Use64 = 0x00A0_0000_0000_0000,
}

impl DefaultSize {
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

    pub const fn code_segment(dpl: PrivilegeLevel, size: DefaultSize) -> Self {
        let value = 0x000F9A000000FFFFu64 | dpl.as_descriptor_entry() | size.as_descriptor_entry();
        DescriptorEntry(value)
    }

    pub const fn data_segment(dpl: PrivilegeLevel) -> Self {
        let value = 0x008F92000000FFFFu64 | dpl.as_descriptor_entry();
        DescriptorEntry(value)
    }

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
    pub const fn new(low: DescriptorEntry, high: DescriptorEntry) -> Self {
        DescriptorPair {
            low: low,
            high: high,
        }
    }
}

#[repr(C, align(16))]
pub struct GlobalDescriptorTable {
    table: [DescriptorEntry; MAX_GDT],
}

impl GlobalDescriptorTable {
    pub fn new(tss: &Box<TaskStateSegment>) -> Box<Self> {
        let tss_pair = DescriptorEntry::tss_descriptor(
            VirtualAddress(tss.as_ref() as *const _ as usize),
            tss.limit(),
        );
        let mut gdt = Box::new(GlobalDescriptorTable {
            table: [DescriptorEntry::null(); MAX_GDT],
        });
        gdt.table[KERNEL_CODE.index()] =
            DescriptorEntry::code_segment(PrivilegeLevel::Kernel, DefaultSize::Use64);
        gdt.table[KERNEL_DATA.index()] = DescriptorEntry::data_segment(PrivilegeLevel::Kernel);
        let tss_index = TSS.index();
        gdt.table[tss_index] = tss_pair.low;
        gdt.table[tss_index + 1] = tss_pair.high;

        unsafe {
            gdt.reload();
        }
        gdt
    }

    // Reload GDT and Segment Selectors
    pub unsafe fn reload(&self) {
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
        ", out(reg) _, in(reg) KERNEL_DATA.0, in(reg) KERNEL_CODE.0);
        asm!("ltr {0:x}", in(reg) TSS.0);
    }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    raw: [DescriptorEntry; MAX_IDT * 2],
}

impl InterruptDescriptorTable {
    const fn new() -> Self {
        InterruptDescriptorTable {
            raw: [DescriptorEntry::null(); MAX_IDT * 2],
        }
    }

    pub fn init() {
        unsafe {
            Self::load();
            Self::register(
                Exception::DivideError.as_vec(),
                VirtualAddress(_int00 as usize),
            );
            Self::register(
                Exception::Breakpoint.as_vec(),
                VirtualAddress(_int03 as usize),
            );
            Self::register(
                Exception::InvalidOpcode.as_vec(),
                VirtualAddress(_int06 as usize),
            );
            Self::register(
                Exception::DoubleFault.as_vec(),
                VirtualAddress(_int08 as usize),
            );
            Self::register(
                Exception::GeneralProtection.as_vec(),
                VirtualAddress(_int0D as usize),
            );
            Self::register(
                Exception::PageFault.as_vec(),
                VirtualAddress(_int0E as usize),
            );
        }
    }

    pub unsafe fn load() {
        asm!("
            push {0}
            push {1}
            lidt [rsp+6]
            add rsp, 16
        ", in(reg) &IDT.raw, in(reg) ((IDT.raw.len() * 8 - 1) << 48));
    }

    pub unsafe fn register(vec: InterruptVector, offset: VirtualAddress) {
        let pair = DescriptorEntry::gate_descriptor(
            offset,
            KERNEL_CODE,
            PrivilegeLevel::Kernel,
            DescriptorType::InterruptGate,
        );
        let table_offset = vec.0 as usize * 2;
        IDT.raw[table_offset + 1] = pair.high;
        IDT.raw[table_offset] = pair.low;
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
    pub qword: u64,
    pub tuple: EaxEdx,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct EaxEdx {
    pub eax: u32,
    pub edx: u32,
}

impl Msr {
    pub unsafe fn write(self, value: u64) {
        let value = MsrResult { qword: value };
        asm!("wrmsr", in("eax") value.tuple.eax, in("edx") value.tuple.edx, in("ecx") self as u32);
    }

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
    pub rflags: Eflags,
    pub rsp: u64,
    pub ss: u16,
    _padding_4: [u16; 3],
}

static mut GLOBAL_EXCEPTION_LOCK: Spinlock = Spinlock::new();

#[no_mangle]
pub extern "C" fn default_int_ex_handler(ctx: *mut X64StackContext) {
    unsafe {
        GLOBAL_EXCEPTION_LOCK.lock();
        let ctx = ctx.as_ref().unwrap();
        stdout().set_cursor_enabled(false);
        stdout().set_attribute(0x1F);
        println!(
            "\n#### EXCEPTION {:02x} {:04x} ip {:02x}:{:016x} sp {:02x}:{:016x} fl {:08x}",
            ctx.vector.0,
            ctx.error_code,
            ctx.cs,
            ctx.rip,
            ctx.ss,
            ctx.rsp,
            ctx.rflags.bits(),
        );
        GLOBAL_EXCEPTION_LOCK.unlock();
        println!(
            "rax {:016x} rbx {:016x} rcx {:016x} rdx {:016x}
rbp {:016x} rsi {:016x} rdi {:016x}
r8  {:016x} r9  {:016x} r10 {:016x} r11 {:016x}
r12 {:016x} r13 {:016x} r14 {:016x} r15 {:016x}",
            ctx.rax,
            ctx.rbx,
            ctx.rcx,
            ctx.rdx,
            ctx.rbp,
            ctx.rsi,
            ctx.rdi,
            ctx.r8,
            ctx.r9,
            ctx.r10,
            ctx.r11,
            ctx.r12,
            ctx.r13,
            ctx.r14,
            ctx.r15,
        );
        loop {
            asm!("hlt");
        }
    }
}
