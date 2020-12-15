// x64 Paging

// use crate::mem::alloc;
use crate::mem::memory::*;
use bitflags::*;
// use bootprot::*;

pub type PhysicalAddress = u64;
type PageTableRepr = u64;

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
struct PageTableEntry {
    repr: PageTableRepr,
}

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
}

impl From<PhysicalAddress> for PageTableEntry {
    fn from(value: PhysicalAddress) -> Self {
        Self {
            repr: value as PageTableRepr,
        }
    }
}
