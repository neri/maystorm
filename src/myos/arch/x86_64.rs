// x86_64 Processor
use core::arch::x86_64::*;

const MAX_GDT: usize = 8;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct LinearAddress(pub u64);

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Selector {
    KernelCode = 0x08,
    KernelData = 0x10,
}

impl Selector {
    pub fn rpl(&self) -> PrivilegeLevel {
        PrivilegeLevel::from(*self as usize)
    }

    pub const fn index(&self) -> u16 {
        (*self as u16) >> 3
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
pub enum GateType {
    InterruptGate = 14,
    TrapGate = 15,
}

impl GateType {
    pub const fn as_descriptor_entry(&self) -> u64 {
        let ty = *self as u64;
        ty << 40
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum InterruptVector {
    Divide = 0,
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
    GeneralProtectionException = 13,
    PageFault = 14,
    //Unavailable = 15,
    FloatingPointException = 16,
    AlignmentCheck = 17,
    MachieCheck = 18,
    SimdException = 19,
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
    pub fn new() -> Self {
        TaskStateSegment {
            stack_pointer: [0; 3],
            ist: [0; 7],
            iomap_base: 0,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
        }
    }
}

pub enum DefaultSize {
    Use16,
    Use32,
    Use64,
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq)]
pub struct DescriptorEntry(pub u64);

impl DescriptorEntry {
    pub const fn null() -> Self {
        Self(0)
    }

    pub const fn code_segment(dpl: PrivilegeLevel) -> Self {
        let value = 0x00AF9A000000FFFFu64 | dpl.as_descriptor_entry();
        DescriptorEntry(value)
    }

    pub const fn data_segment(dpl: PrivilegeLevel) -> Self {
        let value = 0x00AF92000000FFFFu64 | dpl.as_descriptor_entry();
        DescriptorEntry(value)
    }

    const fn gate_descriptor_low(
        offset: LinearAddress,
        sel: Selector,
        dpl: PrivilegeLevel,
        ty: GateType,
    ) -> Self {
        let offset = offset.0;
        let value = 0x8000_0000_0000
            | (offset & 0xFFFF)
            | (sel as u64) << 16
            | dpl.as_descriptor_entry()
            | ty.as_descriptor_entry()
            | (offset & 0xFFFF_0000) << 32;
        DescriptorEntry(value)
    }

    const fn gate_descriptor_high(offset: LinearAddress) -> Self {
        DescriptorEntry(offset.0 >> 32)
    }

    pub const fn gate_descriptor(
        offset: LinearAddress,
        sel: Selector,
        dpl: PrivilegeLevel,
        ty: GateType,
    ) -> DescriptorPair {
        DescriptorPair::new(
            Self::gate_descriptor_low(offset, sel, dpl, ty),
            Self::gate_descriptor_high(offset),
        )
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
    pub fn new() -> Self {
        GlobalDescriptorTable {
            table: [
                DescriptorEntry::null(),
                DescriptorEntry::code_segment(PrivilegeLevel::Kernel),
                DescriptorEntry::data_segment(PrivilegeLevel::Kernel),
                DescriptorEntry::null(),
                DescriptorEntry::null(),
                DescriptorEntry::null(),
                DescriptorEntry::null(),
                DescriptorEntry::null(),
            ],
        }
    }

    // Reload GDT and Segment Selectors
    pub unsafe fn reload(&self) {
        llvm_asm!("
            push %rax
            push %rcx
            lgdt 6(%rsp)
            mov %rsp, %rax
            push $$0x0
            push %rax
            pushfq
            push %rdx
            .byte 0xE8, 2, 0, 0, 0, 0xEB, 0x02, 0x48, 0xCF
            add $$0x10, %rsp
            mov %r8d, %ds
            mov %r8d, %es
            "
        :
        : "{rax}"(&self.table), "{rcx}"((self.table.len() * 8 - 1) << 48),
         "{rdx}"(Selector::KernelCode), "{r8}"(Selector::KernelData)
        );
    }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    raw: [DescriptorEntry; 512],
}

impl InterruptDescriptorTable {
    pub const fn new() -> Self {
        InterruptDescriptorTable {
            raw: [DescriptorEntry::null(); 512],
        }
    }

    pub fn init() {
        unsafe {
            Self::load();
            Self::write(
                InterruptVector::GeneralProtectionException,
                LinearAddress(interrupt_foo_handler as usize as u64),
                PrivilegeLevel::Kernel,
                false,
            );
        }
    }

    pub unsafe fn load() {
        llvm_asm!("
            push %rax
            push %rcx
            lidt 6(%rsp)
            add $$0x10, %rsp
            "
            :
            : "{rax}"(&IDT.raw), "{rcx}"((IDT.raw.len() * 8 - 1) << 48)
        );
    }

    pub unsafe fn write(
        index: InterruptVector,
        offset: LinearAddress,
        dpl: PrivilegeLevel,
        is_trap: bool,
    ) {
        let pair = DescriptorEntry::gate_descriptor(
            offset,
            Selector::KernelCode,
            dpl,
            if is_trap {
                GateType::TrapGate
            } else {
                GateType::InterruptGate
            },
        );
        let table_offset = index as usize * 2;
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

extern "x86-interrupt" fn interrupt_foo_handler(
    stack_frame: &ExceptionStackFrame,
    error_code: u64,
) {
    unsafe { llvm_asm!("mov $$0xdeadbeef, %eax":::"%eax") };
    panic!(
        "GENERAL PROTECTION FAULT {:04x} {:?}",
        error_code, stack_frame,
    );
}
