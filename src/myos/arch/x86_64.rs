// x86_64 Processor

use alloc::boxed::Box;
// use core::arch::x86_64::*;

const MAX_GDT: usize = 8;
const MAX_IDT: usize = 256;
const KERNEL_CODE: Selector = Selector::new(1, PrivilegeLevel::Kernel);
const KERNEL_DATA: Selector = Selector::new(2, PrivilegeLevel::Kernel);
const TSS: Selector = Selector::new(6, PrivilegeLevel::Kernel);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct LinearAddress(pub usize);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Limit(pub u16);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Selector(pub u16);

impl Selector {
    pub const fn new(index: usize, rpl: PrivilegeLevel) -> Self {
        Selector((index << 3) as u16 | rpl as u16)
    }

    pub fn rpl(&self) -> PrivilegeLevel {
        PrivilegeLevel::from(self.0 as usize)
    }

    pub const fn index(&self) -> usize {
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
    pub const fn as_descriptor_entry(&self) -> u64 {
        let dpl = *self as u64;
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
    pub const fn as_descriptor_entry(&self) -> u64 {
        let ty = *self as u64;
        ty << 40
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct InterruptVector(pub u8);

impl core::ops::Add<isize> for InterruptVector {
    type Output = Self;
    fn add(self, rhs: isize) -> Self {
        Self((self.0 as isize + rhs as isize) as u8)
    }
}

#[repr(u8)]
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
    pub const fn as_vec(&self) -> InterruptVector {
        InterruptVector(*self as u8)
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
    pub const fn as_descriptor_entry(&self) -> u64 {
        *self as u64
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
    pub const fn is_null(&self) -> bool {
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

    pub const fn tss_descriptor(offset: LinearAddress, limit: Limit) -> DescriptorPair {
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
        offset: LinearAddress,
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
            LinearAddress(tss.as_ref() as *const _ as usize),
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
        llvm_asm!("
            push $0
            push $1
            lgdt 6(%rsp)
            add $$0x10, %rsp
            "
            ::"r"(&self.table), "r"((self.table.len() * 8 - 1) << 48)
        );
        llvm_asm!("
            mov %rsp, %rax
            push %rdx
            push %rax
            pushfq
            push %rcx
            .byte 0xE8, 2, 0, 0, 0, 0xEB, 0x02, 0x48, 0xCF
            mov %edx, %ds
            mov %edx, %es
            mov %edx, %fs
            mov %edx, %gs
            "
            ::"{rcx}"(KERNEL_CODE), "{rdx}"(KERNEL_DATA)
            :"%rax"
        );
        llvm_asm!("ltr $0"::"r"(TSS));
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
                Exception::DoubleFault.as_vec(),
                LinearAddress(interrupt_df_handler as usize),
            );
            Self::register(
                Exception::GeneralProtection.as_vec(),
                LinearAddress(interrupt_gp_handler as usize),
            );
            Self::register(
                Exception::PageFault.as_vec(),
                LinearAddress(interrupt_page_handler as usize),
            );
        }
    }

    pub unsafe fn load() {
        llvm_asm!("
            push $0
            push $1
            lidt 6(%rsp)
            add $$0x10, %rsp
            "
            :: "r"(&IDT.raw), "r"((IDT.raw.len() * 8 - 1) << 48)
        );
    }

    pub unsafe fn register(vec: InterruptVector, offset: LinearAddress) {
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

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ExceptionStackFrame {
    pub rip: LinearAddress,
    pub cs: u64,
    pub flags: u64,
    pub rsp: LinearAddress,
    pub ss: u64,
}

extern "x86-interrupt" fn interrupt_df_handler(
    stack_frame: &ExceptionStackFrame,
    _error_code: u64,
) {
    panic!("DOUBLE FAULT {:?}", stack_frame,);
}

extern "x86-interrupt" fn interrupt_gp_handler(stack_frame: &ExceptionStackFrame, error_code: u64) {
    panic!(
        "GENERAL PROTECTION FAULT {:04x} {:?}",
        error_code, stack_frame,
    );
}

extern "x86-interrupt" fn interrupt_page_handler(
    stack_frame: &ExceptionStackFrame,
    error_code: u64,
) {
    let mut cr2: u64;
    unsafe {
        llvm_asm!("mov %cr2, $0":"=r"(cr2));
    }
    panic!(
        "PAGE FAULT {:04x} {:016x} {:?}",
        error_code, cr2, stack_frame,
    );
}
