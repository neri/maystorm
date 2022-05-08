//! Minimal Page Manager

use crate::*;
use bitflags::*;
use bootprot::*;
use core::{intrinsics::*, ops::*, ptr, slice};
use uefi::table::boot::*;

pub struct PageConfig {}

impl PageConfig {
    /// UEFI virtual page size (not the actual page size)
    pub const UEFI_PAGE_SIZE: u64 = 0x0000_1000;
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

    /// First initialize before exit_boot_services
    pub unsafe fn init_first(bs: &BootServices) -> Result<(), Status> {
        let max_address = 0xFFFF_0000;
        let page_size = 0x0020_0000;
        let count = page_size / PageConfig::UEFI_PAGE_SIZE;
        let page_base = match bs.allocate_pages(
            AllocateType::MaxAddress(max_address),
            MemoryType::LOADER_DATA,
            count as usize,
        ) {
            Ok(v) => v,
            Err(err) => return Err(err.status()),
        };
        let shared = Self::shared();
        shared.static_start = page_base;
        shared.static_free = page_size;
        Ok(())
    }

    /// Second initialize after exit_boot_services
    pub unsafe fn init_late(
        info: &mut BootInfo,
        mm: impl ExactSizeIterator<Item = &'static MemoryDescriptor>,
    ) {
        let shared = Self::shared();

        let mm_len = mm.len();
        let buffer = Self::alloc_pages(
            (size_of::<BootMemoryMapDescriptor>() * mm_len + PageConfig::UEFI_PAGE_SIZE as usize
                - 1)
                / PageConfig::UEFI_PAGE_SIZE as usize,
        ) as usize as *const BootMemoryMapDescriptor
            as *mut BootMemoryMapDescriptor;
        info.mmap_base = buffer as usize as u32;
        let buffer = slice::from_raw_parts_mut(buffer, mm_len);
        let mut write_cursor = 0;
        let mut read_cursor = 0;

        let mut last_pa_4g = 0;
        let mut total_memory_size: u64 = 0;
        for mem_desc in mm {
            let mut has_to_copy = true;
            let page_base = mem_desc.phys_start;
            let page_size = mem_desc.page_count * PageConfig::UEFI_PAGE_SIZE;
            let last_pa = page_base + page_size;
            if mem_desc.ty.is_countable() {
                total_memory_size += page_size;
                if last_pa < u32::MAX.into() && last_pa > last_pa_4g {
                    last_pa_4g = last_pa;
                }
            }
            if mem_desc.ty.is_conventional_at_runtime() {
                if last_pa <= PageConfig::MAX_REAL_MEMORY {
                    let base = page_base / 0x1000;
                    let count = page_size / 0x1000;
                    let limit = core::cmp::min(base + count, 256);
                    for i in base..limit {
                        let index = i as usize / 32;
                        let bit = 1 << (i & 31);
                        info.real_bitmap[index] |= bit;
                    }
                    has_to_copy = false;
                }
            }
            let boot_mem_desc = BootMemoryMapDescriptor {
                base: page_base,
                page_count: mem_desc.page_count as u32,
                mem_type: mem_desc.ty.as_boot_memory_type(),
            };

            if has_to_copy {
                if write_cursor == 0 {
                    buffer[write_cursor] = boot_mem_desc;
                    write_cursor += 1;
                } else {
                    let prev_mem_desc = &buffer[read_cursor];
                    let prev_last_pa = prev_mem_desc.base
                        + prev_mem_desc.page_count as u64 * PageConfig::UEFI_PAGE_SIZE;

                    if prev_mem_desc.mem_type == BootMemoryType::Available
                        && boot_mem_desc.mem_type == BootMemoryType::Available
                        && prev_last_pa == boot_mem_desc.base
                    {
                        buffer[read_cursor].page_count += boot_mem_desc.page_count;
                    } else {
                        read_cursor = write_cursor;
                        buffer[write_cursor] = boot_mem_desc;
                        write_cursor += 1;
                    }
                }
            }
        }
        info.total_memory_size = total_memory_size;
        info.mmap_len = write_cursor as u32 + 1;

        // Minimal Paging
        let common_attributes = PageAttributes::from(MProtect::all());

        let cr3 = Self::alloc_pages(1);
        shared.master_cr3 = cr3;
        info.master_cr3 = cr3;
        let pml4 = PageTableEntry::from(cr3).table(1);

        // 0000_0000_0000_0000 - 0000_0000_FFFF_FFFF Identity Mapping (<4G)
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

        // vram (temp)
        let vram_base = info.vram_base;
        let vram_size = Self::pages(
            info.vram_stride as u64 * info.screen_height as u64 * 4,
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

    #[allow(dead_code)]
    fn debug_unit(val: usize) -> (usize, char) {
        if val < 0x0020_0000 {
            (val >> 10, 'K')
        } else if val < 0x8000_0000 {
            (val >> 20, 'M')
        } else {
            (val >> 30, 'G')
        }
    }

    fn shared() -> &'static mut Self {
        unsafe { &mut PM }
    }

    fn alloc_pages(pages: usize) -> PhysicalAddress {
        let shared = Self::shared();
        let size = pages as u64 * PageTableEntry::NATIVE_PAGE_SIZE;
        unsafe {
            let result = atomic_xadd(&mut shared.static_start, size) as PhysicalAddress;
            if shared.static_free <= size {
                panic!("Out of memory");
            }
            atomic_xsub(&mut shared.static_free, size);
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
    fn as_boot_memory_type(&self) -> BootMemoryType;
}
impl MemoryTypeHelper for MemoryType {
    #[inline]
    fn is_conventional_at_runtime(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => true,
            _ => false,
        }
    }

    #[inline]
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

    #[inline]
    fn as_boot_memory_type(&self) -> BootMemoryType {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => BootMemoryType::Available,
            MemoryType::LOADER_CODE => BootMemoryType::OsLoaderCode,
            MemoryType::LOADER_DATA => BootMemoryType::OsLoaderData,
            MemoryType::ACPI_RECLAIM => BootMemoryType::AcpiReclaim,
            MemoryType::ACPI_NON_VOLATILE => BootMemoryType::AcpiNonVolatile,
            MemoryType::MMIO => BootMemoryType::Mmio,
            MemoryType::MMIO_PORT_SPACE => BootMemoryType::MmioPortSpace,
            MemoryType::RESERVED => BootMemoryType::Reserved,
            MemoryType::UNUSABLE => BootMemoryType::Unavailable,
            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::PAL_CODE => {
                BootMemoryType::FirmwareCode
            }
            _ => BootMemoryType::FirmwareData,
        }
    }
}

bitflags! {
    pub struct MProtect: usize {
        const READ  = 0x4;
        const WRITE = 0x2;
        const EXEC  = 0x1;
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
