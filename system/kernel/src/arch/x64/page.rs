use crate::{mem::*, *};
use bitflags::*;
use core::{
    alloc::Layout,
    arch::asm,
    ffi::c_void,
    mem::transmute,
    num::NonZeroUsize,
    ops::{AddAssign, BitOrAssign, SubAssign},
    sync::atomic::*,
};

type PageTableRepr = u64;

/// Page Manager
pub struct PageManager;

impl PageManager {
    const PAGE_SIZE_MIN: usize = 0x1000;
    // const PAGE_SIZE_2M: usize = 0x200000;
    const PAGE_SIZE_M1: PageTableRepr = 0xFFF;
    const PAGE_SIZE_2M_M1: PageTableRepr = 0x1F_FFFF;
    const PAGE_SYSTEM_PREFIX: usize = 0xFFFF_0000_0000_0000;

    const PAGE_USER_MIN: usize = 0x000;
    const PAGE_USER_MAX: usize = 0x100;
    const PAGE_RECURSIVE: usize = 0x1FE;
    // const PAGE_KERNEL_HEAP: usize = 0x1FC;
    const PAGE_DIRECT_MAP: usize = 0x180;
    const DIRECT_BASE: usize = Self::PAGE_SYSTEM_PREFIX | (Self::PAGE_DIRECT_MAP << 39);
    // const HEAP_BASE: usize = Self::PAGE_SYSTEM_PREFIX | (Self::PAGE_KERNEL_HEAP << 39);

    #[inline]
    pub unsafe fn init(_info: &BootInfo) {
        let base = Self::read_pdbr() & !Self::PAGE_SIZE_M1;
        let p = base as usize as *mut PageTableEntry;

        // FFFF_FF00_0000_0000 - FFFF_FF7F_FFFF_FFFF RECURSIVE PAGE TABLE AREA
        p.add(Self::PAGE_RECURSIVE)
            .write_volatile(PageTableEntry::new(
                PhysicalAddress::new(base),
                PageAttributes::NO_EXECUTE | PageAttributes::WRITE | PageAttributes::PRESENT,
            ));

        // FFFF_????_????_???? (TEMP) DIRECT MAPPING AREA
        {
            let mut pte = p.read_volatile();
            pte += PageAttributes::NO_EXECUTE | PageAttributes::WRITE | PageAttributes::PRESENT;
            p.add(Self::PAGE_DIRECT_MAP).write_volatile(pte);
        }

        Self::invalidate_all_pages();
    }

    #[inline]
    pub unsafe fn init_late() {
        // let base = Self::read_pdbr().as_usize() & !(Self::PAGE_SIZE_MIN - 1);
        // let p = base as *const u64 as *mut PageTableEntry;
        // p.write_volatile(PageTableEntry::empty());
        // Self::invalidate_all_pages();
    }

    #[inline]
    pub(super) unsafe fn invalidate_all_pages() {
        Self::write_pdbr(Self::read_pdbr());
    }

    #[inline]
    #[track_caller]
    pub(crate) unsafe fn mmap(request: MemoryMapRequest) -> usize {
        match request {
            MemoryMapRequest::Mmio(base, len) => {
                let Some(len) = NonZeroUsize::new(len) else { return 0 };
                let pa = base as PhysicalAddress;
                let va = Self::direct_map(base);
                Self::_map(
                    va,
                    len,
                    PageTableEntry::new(
                        pa,
                        PageAttributes::NO_EXECUTE
                            | PageAttributes::WRITE
                            | PageAttributes::PRESENT,
                    ),
                )
                .unwrap();
                va
            }
            MemoryMapRequest::Vram(base, len) => {
                let Some(len) = NonZeroUsize::new(len) else { return 0 };
                let pa = base as PhysicalAddress;
                let va = Self::direct_map(base);
                Self::_map(
                    va,
                    len,
                    PageTableEntry::new(
                        pa,
                        PageAttributes::NO_EXECUTE
                            | PageAttributes::WRITE
                            | PageAttributes::USER
                            | PageAttributes::PRESENT,
                    ),
                )
                .unwrap();
                va
            }
            MemoryMapRequest::User(va, len, attr) => {
                if PageLevel::MAX.component(va) < Self::PAGE_USER_MIN
                    || PageLevel::MAX.component(va) >= Self::PAGE_USER_MAX
                    || PageLevel::MAX.component(len) >= (Self::PAGE_USER_MAX - Self::PAGE_USER_MIN)
                    || PageLevel::MAX.component(va + len) >= Self::PAGE_USER_MAX
                {
                    return 0;
                }
                let Some(len) = NonZeroUsize::new(len) else { return 0 };
                let Some(pa) = MemoryManager::alloc_pages(len.get()).map(|v| v.get()) else { return 0 };

                let mut template = PageAttributes::from(attr);
                template.insert(PageAttributes::USER);
                template.set_avl(PageTableAvl::Reserved);
                // template.remove(PageAttributes::PRESENT);

                Self::_map(va, len, PageTableEntry::new(pa, template)).unwrap();
                va
            }
            MemoryMapRequest::MProtect(va, len, attr) => {
                let Some(len) = NonZeroUsize::new(len) else { return 0 };

                Self::_mprotect(va, len, attr)
                    .map(|_| va)
                    .unwrap_or_default()
            }
            MemoryMapRequest::Kernel(_va, _len, _attr) => {
                todo!()
            }
        }
    }

    #[track_caller]
    unsafe fn _map(va: usize, len: NonZeroUsize, template: PageTableEntry) -> Result<(), usize> {
        let mask_4k = Self::PAGE_SIZE_M1;
        let mask_2m = Self::PAGE_SIZE_2M_M1;
        let len = (len.get() + mask_4k as usize) & !(mask_4k) as usize;

        if (va as PageTableRepr & mask_4k) != 0 {
            panic!("mmap: Invalid address: {:016x}", va);
        }

        if template.contains(PageAttributes::LARGE)
            && (va & mask_2m as usize) == 0
            && (len & mask_2m as usize) == 0
            && (mask_2m & template.frame_address()) == 0
        {
            // 2M Pages
            return Err(va);
        } else {
            // 4K Pages
            let count = len / Self::PAGE_SIZE_MIN;
            let mut template = template;
            template.remove(PageAttributes::LARGE);
            let fva = va;
            let mut va = va;
            for _ in 0..count {
                let mut parent_template = template;
                parent_template.insert(PageAttributes::PRESENT | PageAttributes::WRITE);
                for level in [PageLevel::Level4, PageLevel::Level3, PageLevel::Level2] {
                    Self::map_table_if_needed(va, level, parent_template);
                }

                let pdte = PageLevel::Level2.pte_of(va).read_volatile();
                if pdte.contains(PageAttributes::LARGE) {
                    panic!(
                        "LARGE PDT {:016x} {:016x} {:016x} {}",
                        va, pdte.0, fva, count
                    );
                }

                let pte = PageLevel::Level1.pte_of(va);
                pte.write_volatile(template);
                Self::invalidate_tlb(va);
                va += Self::PAGE_SIZE_MIN;
                template += Self::PAGE_SIZE_MIN;
            }
        }
        Ok(())
    }

    #[inline]
    unsafe fn map_table_if_needed(va: usize, level: PageLevel, template: PageTableEntry) {
        let pte = level.pte_of(va);
        if !pte.read_volatile().is_present() {
            let pa = MemoryManager::pg_alloc(Layout::from_size_align_unchecked(
                Self::PAGE_SIZE_MIN,
                Self::PAGE_SIZE_MIN,
            ))
            .unwrap()
            .get() as PhysicalAddress;
            let table = pa.direct_map::<c_void>();
            table.write_bytes(0, Self::PAGE_SIZE_MIN);
            pte.write_volatile(PageTableEntry::new(
                pa as PhysicalAddress,
                template.attributes(),
            ));
            Self::invalidate_tlb(va);
        }
    }

    unsafe fn _mprotect(va: usize, len: NonZeroUsize, attr: MProtect) -> Result<(), usize> {
        let mask_4k = Self::PAGE_SIZE_M1;
        let len = (len.get() + mask_4k as usize) & !(mask_4k) as usize;

        let count = len / Self::PAGE_SIZE_MIN;
        let mut new_attr = PageAttributes::from(attr);
        new_attr.remove(PageAttributes::LARGE);
        let mut parent_template = new_attr;
        parent_template.insert(PageAttributes::WRITE);

        let mut va = va;
        for _ in 0..count {
            for level in [PageLevel::Level4, PageLevel::Level3, PageLevel::Level2] {
                let entry = &mut *level.pte_of(va);
                if !entry.is_present() {
                    return Err(va);
                }
                entry.upgrade(parent_template);
                Self::invalidate_tlb(va);
            }

            let pte = &mut *PageLevel::Level1.pte_of(va);
            pte.set_access_rights(new_attr);

            Self::invalidate_tlb(va);
            va += Self::PAGE_SIZE_MIN;
        }

        Ok(())
    }

    #[inline]
    unsafe fn invalidate_tlb(p: usize) {
        fence(Ordering::SeqCst);
        asm!("invlpg [{}]", in(reg) p);
    }

    // #[inline]
    // pub(super) unsafe fn invalidate_cache(p: usize) {
    //     fence(Ordering::SeqCst);
    //     asm!("clflush [{}]", in(reg) p);
    // }

    #[inline]
    unsafe fn read_pdbr() -> PageTableRepr {
        let result: PageTableRepr;
        asm!("mov {}, cr3", out(reg) result);
        result
    }

    #[inline]
    unsafe fn write_pdbr(val: PageTableRepr) {
        asm!("mov cr3, {}", in(reg) val);
    }

    #[inline]
    pub(super) const fn direct_map(pa: PhysicalAddress) -> usize {
        Self::DIRECT_BASE + pa.as_usize()
    }

    #[inline]
    pub(super) const fn direct_unmap(va: usize) -> PhysicalAddress {
        PhysicalAddress::from_usize(va - Self::DIRECT_BASE)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PageAttributes: PageTableRepr {
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
        const AVL_MASK      = 0x0000_0000_0000_0E00;
        const LARGE_PAT     = 0x0000_0000_0000_1000;
        const NO_EXECUTE    = 0x8000_0000_0000_0000;
    }
}

#[repr(u64)]
#[allow(dead_code)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PageTableAvl {
    Free = 0,
    Reserved = 1,
}

#[allow(dead_code)]
impl PageAttributes {
    pub const AVL_SHIFT: usize = 9;

    #[inline]
    const fn avl(self) -> PageTableAvl {
        unsafe { transmute((self.bits() & Self::AVL_MASK.bits()) >> Self::AVL_SHIFT) }
    }

    #[inline]
    fn set_avl(&mut self, avl: PageTableAvl) {
        *self = Self::from_bits_retain(
            (self.bits() & !Self::AVL_MASK.bits()) | ((avl as PageTableRepr) << Self::AVL_SHIFT),
        );
    }
}

impl From<MProtect> for PageAttributes {
    #[inline]
    fn from(value: MProtect) -> Self {
        match value {
            MProtect::None => PageAttributes::empty(),
            MProtect::Read => PageAttributes::PRESENT | PageAttributes::NO_EXECUTE,
            MProtect::ReadWrite => {
                PageAttributes::PRESENT | PageAttributes::WRITE | PageAttributes::NO_EXECUTE
            }
            MProtect::ReadExec => PageAttributes::PRESENT,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, PartialOrd)]
pub(super) struct PageTableEntry(PageTableRepr);

#[allow(dead_code)]
impl PageTableEntry {
    pub const ADDRESS_BIT: PageTableRepr = 0x0000_FFFF_FFFF_F000;
    pub const NORMAL_ATTRIBUTE_BITS: PageTableRepr = 0x8000_0000_0000_0FFF;

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn new(base: PhysicalAddress, attr: PageAttributes) -> Self {
        Self((base.as_u64() & Self::ADDRESS_BIT) | attr.bits())
    }

    #[inline]
    pub const fn repr(&self) -> PageTableRepr {
        self.0
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_present(&self) -> bool {
        self.contains(PageAttributes::PRESENT)
    }

    #[inline]
    pub const fn contains(&self, flags: PageAttributes) -> bool {
        (self.0 & flags.bits()) == flags.bits()
    }

    #[inline]
    pub const fn insert(&mut self, flags: PageAttributes) {
        self.0 |= flags.bits();
    }

    #[inline]
    pub const fn remove(&mut self, flags: PageAttributes) {
        self.0 &= !flags.bits();
    }

    #[inline]
    pub const fn set(&mut self, flags: PageAttributes, value: bool) {
        match value {
            true => self.insert(flags),
            false => self.remove(flags),
        }
    }

    #[inline]
    pub const fn frame_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0 & Self::ADDRESS_BIT)
    }

    #[inline]
    pub const fn attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_retain(self.0 & Self::NORMAL_ATTRIBUTE_BITS)
    }

    #[inline]
    pub const fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.0 = (pa.as_u64() & Self::ADDRESS_BIT) | (self.0 & !Self::ADDRESS_BIT);
    }

    #[inline]
    pub const fn set_attributes(&mut self, flags: PageAttributes) {
        self.0 = (self.0 & Self::ADDRESS_BIT) | (flags.bits() & !Self::ADDRESS_BIT);
    }

    #[inline]
    pub fn set_access_rights(&mut self, new_attr: PageAttributes) {
        for flag in [PageAttributes::NO_EXECUTE, PageAttributes::WRITE] {
            self.set(flag, new_attr.contains(flag));
        }
    }

    #[inline]
    pub fn upgrade(&mut self, new_attr: PageAttributes) {
        if !new_attr.contains(PageAttributes::NO_EXECUTE) {
            self.remove(PageAttributes::NO_EXECUTE)
        }
        if new_attr.contains(PageAttributes::WRITE) {
            self.insert(PageAttributes::WRITE)
        }
    }
}

impl const AddAssign<PageAttributes> for PageTableEntry {
    #[inline]
    fn add_assign(&mut self, rhs: PageAttributes) {
        self.insert(rhs);
    }
}

impl const SubAssign<PageAttributes> for PageTableEntry {
    #[inline]
    fn sub_assign(&mut self, rhs: PageAttributes) {
        self.remove(rhs);
    }
}

impl const BitOrAssign<PageAttributes> for PageTableEntry {
    #[inline]
    fn bitor_assign(&mut self, rhs: PageAttributes) {
        self.insert(rhs);
    }
}

impl const AddAssign<usize> for PageTableEntry {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        let pa = self.frame_address() + rhs;
        self.set_frame_address(pa);
    }
}

impl const From<PhysicalAddress> for PageTableEntry {
    #[inline]
    fn from(value: PhysicalAddress) -> Self {
        Self(value.as_u64())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PageLevel {
    /// The lowest level of the Page Table.
    Level1 = 1,
    /// The official name is "Page Directory Table"
    Level2,
    /// The official name is "Page Directory Pointer Table"
    Level3,
    /// The top level page table in 4-level paging, officially named "Page Map Level 4 Table".
    Level4,
}

#[allow(dead_code)]
impl PageLevel {
    pub const MIN: Self = Self::Level1;
    pub const MAX: Self = Self::Level4;

    pub const MASK_MAX_VA: usize = 0x0000_FFFF_FFFF_FFFF;
    pub const MASK_PER_LEVEL: usize = 0x1FF;
    pub const BITS_PER_LEVEL: usize = 9;
    pub const FIRST_LEVEL_BITS: usize = 12;

    pub const RECURSIVE_LV1: usize =
        PageManager::PAGE_SYSTEM_PREFIX | (PageManager::PAGE_RECURSIVE << 39);
    pub const RECURSIVE_LV2: usize = Self::RECURSIVE_LV1 | (PageManager::PAGE_RECURSIVE << 30);
    pub const RECURSIVE_LV3: usize = Self::RECURSIVE_LV2 | (PageManager::PAGE_RECURSIVE << 21);
    pub const RECURSIVE_LV4: usize = Self::RECURSIVE_LV3 | (PageManager::PAGE_RECURSIVE << 12);

    /// Returns the component of the current level specified by linear address.
    #[inline]
    pub const fn component(&self, va: usize) -> usize {
        (va >> (Self::FIRST_LEVEL_BITS
            + Self::BITS_PER_LEVEL
                * match *self {
                    Self::Level1 => 0,
                    Self::Level2 => 1,
                    Self::Level3 => 2,
                    Self::Level4 => 3,
                }))
            & Self::MASK_PER_LEVEL
    }

    /// Returns the PageTableEntry corresponding to the current level of the specified linear address.
    #[inline]
    pub const unsafe fn pte_of(&self, va: usize) -> *mut PageTableEntry {
        let base = va & Self::MASK_MAX_VA;
        let pte = match *self {
            Self::Level1 => Self::RECURSIVE_LV1 + ((base >> 12) << 3),
            Self::Level2 => Self::RECURSIVE_LV2 + ((base >> 21) << 3),
            Self::Level3 => Self::RECURSIVE_LV3 + ((base >> 30) << 3),
            Self::Level4 => Self::RECURSIVE_LV4 + ((base >> 39) << 3),
        };
        pte as *mut PageTableEntry
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PageErrorCode: u16 {
        /// When set, the page fault was caused by a page-protection violation.
        /// When not set, it was caused by a non-present page.
        const PRESENT           = 0b0000_0000_0000_0001;
        /// When set, the page fault was caused by a write access.
        /// When not set, it was caused by a read access.
        const WRITE             = 0b0000_0000_0000_0010;
        /// When set, the page fault was caused while CPL = 3.
        /// This does not necessarily mean that the page fault was a privilege violation.
        const USER              = 0b0000_0000_0000_0100;
        /// When set, one or more page directory entries contain reserved bits which are set to 1.
        /// This only applies when the PSE or PAE flags in CR4 are set to 1.
        const RESERVED_BITS     = 0b0000_0000_0000_1000;
        /// When set, the page fault was caused by an instruction fetch.
        /// This only applies when the No-Execute bit is supported and enabled.
        const FETCH             = 0b0000_0000_0001_0000;
        /// When set, the page fault was caused by a protection-key violation.
        /// The PKRU register (for user-mode accesses) or PKRS MSR (for supervisor-mode accesses) specifies the protection key rights.
        const PROTECTION_KEY    = 0b0000_0000_0010_0000;
        /// When set, the page fault was caused by a shadow stack access.
        const SHADOW_STACK      = 0b0000_0000_0100_0000;
        /// When set, the fault was due to an SGX violation.
        /// The fault is unrelated to ordinary paging.
        const SGX               = 0b1000_0000_0000_0000;
    }
}

impl PageErrorCode {
    #[inline]
    pub fn is_page_present(&self) -> bool {
        self.contains(Self::PRESENT)
    }

    #[inline]
    pub fn could_not_read(&self) -> bool {
        !self.could_not_write() && !self.could_not_execute()
    }

    #[inline]
    pub fn could_not_write(&self) -> bool {
        self.contains(Self::WRITE)
    }

    #[inline]
    pub fn could_not_execute(&self) -> bool {
        self.contains(Self::FETCH)
    }
}
