// x64 Paging

// use crate::mem::alloc;
use crate::mem::*;
use bitflags::*;
use bootprot::*;

pub type PhysicalAddress = u64;
type PageTableRepr = u64;

pub(crate) struct PageManager {
    _phantom: (),
}

impl PageManager {
    const PAGE_SIZE_MIN: usize = 0x1000;
    const PAGE_RECURSIVE: usize = 0x1FE;
    const PAGE_DIRECT_MAP: usize = 0x180;
    const DIRECT_BASE: usize = 0xFFFF_0000_0000_0000 | (Self::PAGE_DIRECT_MAP << 39);

    #[inline]
    pub(crate) unsafe fn init(_info: &BootInfo) {
        let base = Self::read_pdbr() as usize & !(Self::PAGE_SIZE_MIN - 1);
        let p = base as *const u64 as *mut PageTableEntry;

        p.add(Self::PAGE_RECURSIVE)
            .write_volatile(PageTableEntry::new(
                base as u64,
                MProtect::READ_WRITE.into(),
            ));

        p.add(Self::PAGE_DIRECT_MAP)
            .write_volatile(p.read_volatile());

        Self::invalidate_all_pages();
    }

    #[inline]
    pub(crate) unsafe fn init_late() {
        // let base = Self::read_pdbr() as usize & !(Self::PAGE_SIZE_MIN - 1);
        // let p = base as *const u64 as *mut PageTableEntry;
        // p.write_volatile(PageTableEntry::empty());
        // Self::invalidate_all_pages();
    }

    #[inline]
    unsafe fn invalidate_all_pages() {
        Self::write_pdbr(Self::read_pdbr());
    }

    #[inline]
    unsafe fn read_pdbr() -> u64 {
        let result: u64;
        asm!("mov {}, cr3", out(reg) result);
        result
    }

    #[inline]
    unsafe fn write_pdbr(val: u64) {
        asm!("mov cr3, {}", in(reg) val);
    }

    #[inline]
    pub const fn direct_map(pa: usize) -> usize {
        Self::DIRECT_BASE + pa
    }
}

bitflags! {
    struct PageAttributes: PageTableRepr {
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

#[allow(dead_code)]
#[non_exhaustive]
enum PageTableAvl {
    None = 0,
    Reserved = 1,
}

#[allow(dead_code)]
impl PageAttributes {
    const AVL_SHIFT: usize = 9;

    #[inline]
    pub const fn avl(self) -> PageTableAvl {
        // ((self.bits() & Self::AVL) >> Self::AVL_SHIFT) // TODO:
        PageTableAvl::None
    }

    #[inline]
    pub fn set_avl(mut self, avl: PageTableAvl) {
        self.bits =
            (self.bits() & !Self::AVL_MASK.bits()) | ((avl as PageTableRepr) << Self::AVL_SHIFT)
    }
}

impl From<MProtect> for PageAttributes {
    fn from(prot: MProtect) -> Self {
        let mut value = PageAttributes::empty();
        if prot.contains(MProtect::READ) {
            value |= PageAttributes::PRESENT;
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
struct PageTableEntry(PageTableRepr);

#[allow(dead_code)]
impl PageTableEntry {
    const ADDRESS_BIT: PageTableRepr = 0x0000_FFFF_FFFF_F000;
    const NATIVE_PAGE_SIZE: PageTableRepr = 0x0000_1000;
    const N_NATIVE_PAGE_ENTRIES: usize = 512;
    const LARGE_PAGE_SIZE: PageTableRepr = 0x0020_0000;
    const INDEX_MASK: usize = 0x1FF;
    const MAX_PAGE_LEVEL: usize = 4;
    const SHIFT_PER_LEVEL: usize = 9;
    const SHIFT_PTE: usize = 3;

    #[inline]
    const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    const fn repr(&self) -> PageTableRepr {
        self.0
    }

    #[inline]
    const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    const fn new(base: PhysicalAddress, attr: PageAttributes) -> Self {
        Self((base & Self::ADDRESS_BIT) | attr.bits())
    }

    #[inline]
    fn contains(&self, flags: PageAttributes) -> bool {
        (self.0 & flags.bits()) == flags.bits()
    }

    #[inline]
    fn insert(&mut self, flags: PageAttributes) {
        self.0 |= flags.bits();
    }

    #[inline]
    fn remove(&mut self, flags: PageAttributes) {
        self.0 &= !flags.bits();
    }

    #[inline]
    fn frame_address(&self) -> PhysicalAddress {
        self.0 & Self::ADDRESS_BIT
    }

    #[inline]
    fn attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    #[inline]
    fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.0 = (pa & Self::ADDRESS_BIT) | (self.0 & !Self::ADDRESS_BIT);
    }

    #[inline]
    fn set_attributes(&mut self, flags: PageAttributes) {
        self.0 = (self.0 & Self::ADDRESS_BIT) | (flags.bits() & !Self::ADDRESS_BIT);
    }
}

impl From<PhysicalAddress> for PageTableEntry {
    #[inline]
    fn from(value: PhysicalAddress) -> Self {
        Self(value as PageTableRepr)
    }
}
