// x86_64 Processor

use alloc::boxed::Box;
use core::arch::x86_64::*;

const MAX_GDT: usize = 8;
const KERNEL_CODE: Selector = Selector::new(1, PrivilegeLevel::Kernel);
const KERNEL_DATA: Selector = Selector::new(2, PrivilegeLevel::Kernel);
const TSS: Selector = Selector::new(6, PrivilegeLevel::Kernel);

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct LinearAddress(pub u64);

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

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum InterruptVector {
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
    GeneralProtectionException = 13,
    PageFault = 14,
    //Unavailable = 15,
    FloatingPointException = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SimdException = 19,
}

impl InterruptVector {
    pub fn mnemonic(&self) -> &str {
        match *self {
            InterruptVector::DivideError => "#DE",
            InterruptVector::Debug => "#DB",
            InterruptVector::Breakpoint => "#BP",
            InterruptVector::Overflow => "#OF",
            InterruptVector::InvalidOpcode => "#UD",
            InterruptVector::DeviceNotAvailable => "#NM",
            InterruptVector::DoubleFault => "#DF",
            InterruptVector::StackException => "#SS",
            InterruptVector::GeneralProtectionException => "#GP",
            InterruptVector::PageFault => "#PF",
            InterruptVector::FloatingPointException => "#MF",
            InterruptVector::AlignmentCheck => "#AC",
            InterruptVector::MachineCheck => "#MC",
            InterruptVector::SimdException => "#XM",
            _ => "??",
        }
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
    pub const fn present() -> Self {
        Self(0x8000_0000_0000)
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
        let offset = offset.0;
        let low = DescriptorEntry(
            Self::present().0
                | DescriptorType::Tss.as_descriptor_entry()
                | limit.0 as u64
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
        let offset = offset.0;
        let low = DescriptorEntry(
            Self::present().0
                | (offset & 0xFFFF)
                | (sel.0 as u64) << 16
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
            LinearAddress(tss.as_ref() as *const _ as u64),
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
            Self::write(
                InterruptVector::PageFault,
                LinearAddress(interrupt_page_handler as usize as u64),
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
            KERNEL_CODE,
            dpl,
            if is_trap {
                DescriptorType::TrapGate
            } else {
                DescriptorType::InterruptGate
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
    panic!(
        "GENERAL PROTECTION FAULT {} {:04x} {:?}",
        InterruptVector::GeneralProtectionException.mnemonic(),
        error_code,
        stack_frame,
    );
}

extern "x86-interrupt" fn interrupt_page_handler(
    stack_frame: &ExceptionStackFrame,
    error_code: u64,
) {
    let mut cr2: u64 = 0;
    unsafe {
        llvm_asm!("mov %cr2, $0":"=r"(cr2));
    }
    panic!(
        "PAGE FAULT {} {:04x} {:016x} {:?}",
        InterruptVector::PageFault.mnemonic(),
        error_code,
        cr2,
        stack_frame,
    );
}
