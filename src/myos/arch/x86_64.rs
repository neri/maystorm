// x86_64 Processor

const MAX_GDT: usize = 8;

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
    pub const fn as_descriptor(&self) -> u64 {
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
    pub const fn as_descriptor(&self) -> u64 {
        let ty = *self as u64;
        ty << 40
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

pub enum DescriptorDefaultSize {
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
        let value = 0x00AF9A000000FFFFu64 | dpl.as_descriptor();
        DescriptorEntry(value)
    }

    pub const fn data_segment(dpl: PrivilegeLevel) -> Self {
        let value = 0x00AF92000000FFFFu64 | dpl.as_descriptor();
        DescriptorEntry(value)
    }

    pub const fn gate_descriptor_low(
        offset: u64,
        sel: Selector,
        dpl: PrivilegeLevel,
        ty: GateType,
    ) -> Self {
        let value = 0x8000_0000_0000
            | (offset & 0xFFFF)
            | (sel as u64) << 16
            | dpl.as_descriptor()
            | ty.as_descriptor()
            | (offset & 0xFFFF_0000) << 32;
        DescriptorEntry(value)
    }

    pub const fn gate_descriptor_high(offset: u64) -> Self {
        DescriptorEntry(offset >> 32)
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

    pub fn write(&mut self, index: u8, desc: DescriptorPair) {
        self.raw[index as usize * 2 + 1] = desc.high;
        self.raw[index as usize * 2] = desc.low;
    }
}
