// Minimal Page Manager

use bitflags::*;
use bootprot::*;
use core::intrinsics::*;
use core::ops::*;
use core::ptr;
use core::slice;
use uefi::table::boot::*;

struct PageConfig {}

impl PageConfig {
    const UEFI_PAGE_SIZE: u64 = 0x0000_1000;
    const N_DIRECT_MAP_GIGA: usize = 4;
    const MAX_REAL_MEMORY: u64 = 0x0000A_0000;
}

type IntPtr = u64;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct VirtualAddress(pub IntPtr);

impl VirtualAddress {
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    pub const fn index_of(&self, level: usize) -> usize {
        (self.0 >> (level * PageTableEntry::SHIFT_PER_LEVEL + PageTableEntry::SHIFT_PTE)) as usize
            & PageTableEntry::INDEX_MASK
    }
}

impl Add<u32> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: u32) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: u64) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Add<usize> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        VirtualAddress(self.0 + rhs as IntPtr)
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self {
        VirtualAddress(self.0 - rhs as IntPtr)
    }
}

pub type PhysicalAddress = u64;

static mut PM: PageManager = PageManager::new();

pub struct PageManager {
    pub master_cr3: PhysicalAddress,
    pub static_start: PhysicalAddress,
    pub static_free: PhysicalAddress,
    pml2k: PageTableEntry,
}

impl PageManager {
    const fn new() -> Self {
        Self {
            master_cr3: 0,
            static_start: 0,
            static_free: 0,
            pml2k: PageTableEntry::empty(),
        }
    }

    pub fn init(info: &mut BootInfo, mm: impl Iterator<Item = &'static MemoryDescriptor>) {
        let shared = Self::shared();

        let mut last_pa_4g = 0;
        let mut static_size = 0;
        let mut total_memory_size: u64 = 0;
        for mem_desc in mm {
            let last_pa = mem_desc.phys_start + mem_desc.page_count * PageConfig::UEFI_PAGE_SIZE;
            if mem_desc.ty.is_countable() {
                total_memory_size += mem_desc.page_count << 12;
                if last_pa < u32::MAX.into() && last_pa > last_pa_4g {
                    last_pa_4g = last_pa;
                }
            }
            if mem_desc.ty.is_conventional_at_runtime() {
                if last_pa <= PageConfig::MAX_REAL_MEMORY {
                    let base = mem_desc.phys_start / 0x1000;
                    let count = mem_desc.page_count;
                    let limit = core::cmp::min(base + count, 256);
                    for i in base..limit {
                        let index = i as usize / 32;
                        let bit = 1 << (i & 31);
                        info.real_bitmap[index] |= bit;
                    }
                }
                if mem_desc.page_count > static_size && last_pa < u32::MAX.into() {
                    static_size = mem_desc.page_count;
                    shared.static_start = mem_desc.phys_start;
                    shared.static_free = static_size * PageConfig::UEFI_PAGE_SIZE;
                }
            }
        }
        info.total_memory_size = total_memory_size;

        // Minimal Paging
        let common_attributes = PageAttributes::from(MProtect::all());

        let cr3 = Self::alloc_pages(1);
        shared.master_cr3 = cr3;
        info.master_cr3 = cr3;
        let pml4 = PageTableEntry::from(cr3).table(1);

        let pml3p = Self::alloc_pages(1);
        let pml3 = PageTableEntry::from(pml3p).table(1);
        pml4[0] = PageTableEntry::new(pml3p, common_attributes);

        let n_pages = PageConfig::N_DIRECT_MAP_GIGA;
        let pml2p = Self::alloc_pages(n_pages);
        let pml2 = PageTableEntry::from(pml2p).table(n_pages);
        for i in 0..n_pages {
            pml3[i] = PageTableEntry::new(
                pml2p + i as PhysicalAddress * PageTableEntry::NATIVE_PAGE_SIZE,
                common_attributes,
            );
        }
        let limit = ((last_pa_4g + PageTableEntry::LARGE_PAGE_SIZE - 1)
            / PageTableEntry::LARGE_PAGE_SIZE) as usize;
        for i in 0..limit {
            pml2[i] = PageTableEntry::new(
                i as PhysicalAddress * PageTableEntry::LARGE_PAGE_SIZE,
                common_attributes | PageAttributes::LARGE,
            );
        }

        // kernel memory
        let kernel_base = VirtualAddress(info.kernel_base);

        let pml3kp = Self::alloc_pages(1);
        let pml3k = PageTableEntry::from(pml3kp).table(1);
        pml4[kernel_base.index_of(4)] = PageTableEntry::new(pml3kp, common_attributes);

        let pml2kp = Self::alloc_pages(1);
        shared.pml2k = PageTableEntry::new(pml2kp, common_attributes);
        pml3k[kernel_base.index_of(3)] = shared.pml2k;

        // TODO: Temp Peripherals
        // FEC00000 IOAPIC
        // FED00000 HPET
        // FEE00000 LocalAPIC
        {
            let la = 0xFEC0_0000;
            let offset = la / PageTableEntry::LARGE_PAGE_SIZE;
            for i in 0..2 {
                pml2[(offset + i) as usize] = PageTableEntry::new(
                    la + i * PageTableEntry::LARGE_PAGE_SIZE,
                    common_attributes | PageAttributes::LARGE,
                );
            }
        }

        // vram (temp)
        let vram_base = info.vram_base;
        let vram_size = Self::pages(
            if info.flags.contains(BootFlags::PORTRAIT) {
                info.vram_stride as u64 * info.screen_width as u64 * 4
            } else {
                info.vram_stride as u64 * info.screen_height as u64 * 4
            },
            PageTableEntry::LARGE_PAGE_SIZE,
        ) as u64;
        let offset = vram_base / PageTableEntry::LARGE_PAGE_SIZE;
        for i in 0..vram_size {
            pml2[(offset + i) as usize] = PageTableEntry::new(
                vram_base + i * PageTableEntry::LARGE_PAGE_SIZE,
                common_attributes | PageAttributes::LARGE,
            );
        }
    }

    pub unsafe fn finalize(info: &mut BootInfo) {
        let shared = Self::shared();
        info.static_start = shared.static_start as u32;
        info.free_memory = shared.static_free as u32;
    }

    fn shared() -> &'static mut Self {
        unsafe { &mut PM }
    }

    fn alloc_pages(pages: usize) -> PhysicalAddress {
        let shared = Self::shared();
        let size = pages as u64 * PageTableEntry::NATIVE_PAGE_SIZE;
        unsafe {
            let result = atomic_xadd(&mut shared.static_start, size) as PhysicalAddress;
            atomic_xadd(&mut shared.static_free, 0 - size);
            let ptr = result as *const u8 as *mut u8;
            ptr::write_bytes(ptr, 0, size as usize);
            result
        }
    }

    fn va_set_l1<'a>(base: VirtualAddress) -> &'a mut PageTableEntry {
        let shared = Self::shared();
        let common_attributes = PageAttributes::from(MProtect::all());

        let page = (base.0 / PageTableEntry::LARGE_PAGE_SIZE) as usize & PageTableEntry::INDEX_MASK;
        let offset =
            (base.0 / PageTableEntry::NATIVE_PAGE_SIZE) as usize & PageTableEntry::INDEX_MASK;
        let pml2k = shared.pml2k.table(1);
        let mut pml1e = pml2k[page];
        if pml1e.is_empty() {
            pml1e = PageTableEntry::new(Self::alloc_pages(1), common_attributes);
            pml2k[page] = pml1e;
        }
        let pml1 = pml1e.table(1);

        &mut pml1[offset]
    }

    pub fn valloc(base: VirtualAddress, size: usize) -> usize {
        let common_attributes = PageAttributes::from(MProtect::READ | MProtect::WRITE);

        let size = Self::pages(size as u64, PageTableEntry::NATIVE_PAGE_SIZE) as u64;
        let blob = Self::alloc_pages(size as usize);

        for i in 0..size {
            let p = Self::va_set_l1(base + i * PageTableEntry::NATIVE_PAGE_SIZE);
            *p = PageTableEntry::new(
                blob + i * PageTableEntry::NATIVE_PAGE_SIZE,
                common_attributes,
            );
        }

        blob as usize
    }

    pub fn vprotect(base: VirtualAddress, size: usize, prot: MProtect) {
        let attributes = PageAttributes::from(prot);
        let size = Self::pages(size as u64, PageTableEntry::NATIVE_PAGE_SIZE) as u64;

        for i in 0..size {
            let p = Self::va_set_l1(base + i * PageTableEntry::NATIVE_PAGE_SIZE);
            p.set_attributes(attributes);
        }
    }

    fn ceil(base: PhysicalAddress, page: PhysicalAddress) -> PhysicalAddress {
        (base + page - 1) & !(page - 1)
    }

    fn pages(base: PhysicalAddress, page_size: PhysicalAddress) -> usize {
        (Self::ceil(base, page_size) / page_size) as usize
    }
}

use uefi::table::boot::MemoryType;
pub trait MemoryTypeHelper {
    fn is_conventional_at_runtime(&self) -> bool;
    fn is_countable(&self) -> bool;
}
impl MemoryTypeHelper for MemoryType {
    fn is_conventional_at_runtime(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => true,
            _ => false,
        }
    }

    fn is_countable(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA
            | MemoryType::ACPI_RECLAIM => true,
            _ => false,
        }
    }
}

bitflags! {
    pub struct MProtect: usize {
        const READ  = 0x1;
        const WRITE = 0x2;
        const EXEC  = 0x4;
        const NONE  = 0x0;
    }
}

bitflags! {
    struct PageAttributes: u64 {
        const PRESENT       = 0x0000_0000_0000_0001;
        const WRITE         = 0x0000_0000_0000_0002;
        const USER          = 0x0000_0000_0000_0004;
        const PWT           = 0x0000_0000_0000_0008;
        const PCD           = 0x0000_0000_0000_0010;
        const ACCESS        = 0x0000_0000_0000_0020;
        const DIRTY         = 0x0000_0000_0000_0040;
        const PAT           = 0x0000_0000_0000_0080;
        const LARGE         = 0x0000_0000_0000_0080;
        const GLOBAL        = 0x0000_0000_0000_0100;
        // const AVL           = 0x0000_0000_0000_0E00;
        const LARGE_PAT     = 0x0000_0000_0000_1000;
        const NO_EXECUTE    = 0x8000_0000_0000_0000;
    }
}

impl From<MProtect> for PageAttributes {
    fn from(prot: MProtect) -> Self {
        let mut value = PageAttributes::empty();
        if prot.contains(MProtect::READ) {
            value |= PageAttributes::PRESENT | PageAttributes::USER;
            if prot.contains(MProtect::WRITE) {
                value |= PageAttributes::WRITE;
            }
            if !prot.contains(MProtect::EXEC) {
                value |= PageAttributes::NO_EXECUTE;
            }
        }
        value
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
struct PageTableEntry {
    repr: u64,
}

#[allow(dead_code)]
impl PageTableEntry {
    const ADDRESS_BIT: u64 = 0x0000_FFFF_FFFF_F000;
    const NATIVE_PAGE_SIZE: u64 = 0x0000_1000;
    const N_NATIVE_PAGE_ENTRIES: usize = 512;
    const LARGE_PAGE_SIZE: u64 = 0x0020_0000;
    const INDEX_MASK: usize = 0x1FF;
    const MAX_PAGE_LEVEL: usize = 4;
    const SHIFT_PER_LEVEL: usize = 9;
    const SHIFT_PTE: usize = 3;

    const fn empty() -> Self {
        Self { repr: 0 }
    }

    fn is_empty(&self) -> bool {
        self.repr == 0
    }

    const fn new(base: PhysicalAddress, attr: PageAttributes) -> Self {
        Self {
            repr: (base & Self::ADDRESS_BIT) | attr.bits(),
        }
    }

    fn contains(&self, flags: PageAttributes) -> bool {
        (self.repr & flags.bits()) == flags.bits()
    }

    fn insert(&mut self, flags: PageAttributes) {
        self.repr |= flags.bits();
    }

    fn remove(&mut self, flags: PageAttributes) {
        self.repr &= !flags.bits();
    }

    fn frame_address(&self) -> PhysicalAddress {
        self.repr & Self::ADDRESS_BIT
    }

    fn attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.repr)
    }

    fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.repr = (pa & Self::ADDRESS_BIT) | (self.repr & !Self::ADDRESS_BIT);
    }

    fn set_attributes(&mut self, flags: PageAttributes) {
        self.repr = (self.repr & Self::ADDRESS_BIT) | (flags.bits() & !Self::ADDRESS_BIT);
    }

    fn table<'a>(&self, pages: usize) -> &'a mut [Self] {
        unsafe {
            slice::from_raw_parts_mut(
                self.frame_address() as usize as *const PageTableEntry as *mut PageTableEntry,
                pages * Self::N_NATIVE_PAGE_ENTRIES,
            )
        }
    }
}

impl From<PhysicalAddress> for PageTableEntry {
    fn from(value: PhysicalAddress) -> Self {
        Self { repr: value }
    }
}
