use super::apic::Apic;
use crate::mem::*;
use bitflags::*;
use bootprot::*;
use core::{
    alloc::Layout,
    arch::asm,
    ffi::c_void,
    fmt,
    mem::transmute,
    num::{NonZeroU64, NonZeroUsize},
    ops::{Add, AddAssign, BitAnd, BitOr, BitOrAssign, Mul, Not, Sub, SubAssign},
    sync::atomic::*,
};

type PageTableRepr = u64;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub const NULL: Self = Self(0);

    // pub const MAX: Self = Self(0x0FFF_FFFF_FFFF);

    #[inline]
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn from_usize(val: usize) -> Self {
        Self(val as u64)
    }

    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    /// Gets a pointer identical to the specified physical address.
    ///
    /// # Safety
    ///
    /// Pointers of this form may not map to some memory.
    #[inline]
    pub const unsafe fn identity_map<T>(&self) -> *mut T {
        self.0 as usize as *mut T
    }

    /// Gets the pointer corresponding to the specified physical address.
    #[inline]
    pub const fn direct_map<T>(&self) -> *mut T {
        PageManager::direct_map(*self) as *mut T
    }
}

impl Default for PhysicalAddress {
    #[inline]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Add<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs as u64)
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = usize;

    #[inline]
    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        (self.0 - rhs.0) as usize
    }
}

impl Sub<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs as u64)
    }
}

impl Mul<usize> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self::Output {
        Self(self.0 * rhs as u64)
    }
}

impl Mul<u64> for PhysicalAddress {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl BitAnd<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: u64) -> Self::Output {
        Self(self.0 & rhs)
    }
}

impl BitAnd<PhysicalAddress> for u64 {
    type Output = Self;

    fn bitand(self, rhs: PhysicalAddress) -> Self::Output {
        self & rhs.0
    }
}

impl BitOr<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: u64) -> Self::Output {
        Self(self.0 | rhs)
    }
}

impl Not for PhysicalAddress {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl From<u64> for PhysicalAddress {
    #[inline]
    fn from(val: u64) -> Self {
        Self::new(val)
    }
}

impl From<PhysicalAddress> for u64 {
    #[inline]
    fn from(val: PhysicalAddress) -> Self {
        val.as_u64()
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:012x}", self.0)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonNullPhysicalAddress(NonZeroU64);

impl NonNullPhysicalAddress {
    #[inline]
    pub const fn get(&self) -> PhysicalAddress {
        PhysicalAddress(self.0.get())
    }

    #[inline]
    pub const fn new(val: PhysicalAddress) -> Option<Self> {
        match NonZeroU64::new(val.as_u64()) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const unsafe fn new_unchecked(val: PhysicalAddress) -> Self {
        Self(NonZeroU64::new_unchecked(val.as_u64()))
    }
}

impl From<NonNullPhysicalAddress> for PhysicalAddress {
    #[inline]
    fn from(val: NonNullPhysicalAddress) -> Self {
        val.get()
    }
}

/// Page Manager
pub struct PageManager;

impl PageManager {
    const PAGE_SIZE_MIN: usize = 0x1000;
    // const PAGE_SIZE_2M: usize = 0x200000;
    const PAGE_SIZE_M1: PageTableRepr = 0xFFF;
    const PAGE_SIZE_2M_M1: PageTableRepr = 0x1F_FFFF;
    const PAGE_KERNEL_PREFIX: usize = 0xFFFF_0000_0000_0000;
    const PAGE_RECURSIVE: usize = 0x1FE;
    // const PAGE_KERNEL_HEAP: usize = 0x1FC;
    const PAGE_DIRECT_MAP: usize = 0x180;
    const DIRECT_BASE: usize = Self::PAGE_KERNEL_PREFIX | (Self::PAGE_DIRECT_MAP << 39);
    // const HEAP_BASE: usize = Self::PAGE_KERNEL_PREFIX | (Self::PAGE_KERNEL_HEAP << 39);

    #[inline]
    pub unsafe fn init(_info: &BootInfo) {
        let base = Self::read_pdbr() & !Self::PAGE_SIZE_M1;
        let p = base.as_usize() as *mut PageTableEntry;

        // FFFF_FF00_0000_0000 - FFFF_FF7F_FFFF_FFFF RECURSIVE PAGE TABLE AREA
        p.add(Self::PAGE_RECURSIVE)
            .write_volatile(PageTableEntry::new(
                base,
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
        // let base = Self::read_pdbr() as usize & !(Self::PAGE_SIZE_MIN - 1);
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
    pub unsafe fn mmap(request: MemoryMapRequest) -> usize {
        use MemoryMapRequest::*;
        match request {
            Mmio(base, len) => {
                let len = match NonZeroUsize::new(len) {
                    Some(v) => v,
                    None => return 0,
                };
                let pa = base as PhysicalAddress;
                let va = Self::direct_map(base);
                Self::map(
                    va,
                    len,
                    PageTableEntry::new(pa, PageAttributes::NO_EXECUTE | PageAttributes::WRITE),
                );
                va
            }
            Vram(base, len) => {
                let len = match NonZeroUsize::new(len) {
                    Some(v) => v,
                    None => return 0,
                };
                let pa = base as PhysicalAddress;
                let va = Self::direct_map(base);
                Self::map(
                    va,
                    len,
                    PageTableEntry::new(
                        pa,
                        PageAttributes::NO_EXECUTE
                            | PageAttributes::GLOBAL
                            | PageAttributes::WRITE
                            | PageAttributes::USER,
                    ),
                );
                va
            }
            // Kernel(_, _, _) => todo!(),
            _ => todo!(),
        }
    }

    #[inline]
    #[track_caller]
    pub(super) unsafe fn map(va: usize, len: NonZeroUsize, template: PageTableEntry) {
        let mask_4k = Self::PAGE_SIZE_M1;
        let mask_2m = Self::PAGE_SIZE_2M_M1;
        let len = (len.get() + mask_4k as usize) & !(mask_4k) as usize;

        if (va as PageTableRepr & mask_4k) != 0 {
            panic!("INVALID VA: {:016x}", va);
        }

        if template.contains(PageAttributes::LARGE)
            && (va & mask_2m as usize) == 0
            && (len & mask_2m as usize) == 0
            && (mask_2m & template.frame_address()) == 0
        {
            // 2M Pages
            todo!();
        } else {
            // 4K Pages
            let count = len / Self::PAGE_SIZE_MIN;
            let mut template = template;
            template += PageAttributes::PRESENT;
            template -= PageAttributes::LARGE;
            let fva = va;
            let mut va = va;
            for _ in 0..count {
                Self::map_table_if_needed(va, PageLevel::Level4, template);
                Self::map_table_if_needed(va, PageLevel::Level3, template);
                Self::map_table_if_needed(va, PageLevel::Level2, template);
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

    #[inline]
    unsafe fn invalidate_tlb(p: usize) {
        fence(Ordering::SeqCst);
        asm!("invlpg [{}]", in(reg) p);
    }

    #[inline]
    pub(crate) unsafe fn invalidate_cache(p: usize) {
        fence(Ordering::SeqCst);
        asm!("clflush [{}]", in(reg) p);
    }

    #[inline]
    unsafe fn read_pdbr() -> PhysicalAddress {
        let result: u64;
        asm!("mov {}, cr3", out(reg) result);
        PhysicalAddress::new(result)
    }

    #[inline]
    unsafe fn write_pdbr(val: PhysicalAddress) {
        asm!("mov cr3, {}", in(reg) val.as_u64());
    }

    #[inline]
    const fn direct_map(pa: PhysicalAddress) -> usize {
        Self::DIRECT_BASE + pa.as_usize()
    }

    #[inline]
    pub const fn direct_unmap(va: usize) -> PhysicalAddress {
        PhysicalAddress::from_usize(va - Self::DIRECT_BASE)
    }

    #[inline]
    pub fn broadcast_invalidate_tlb() -> Result<(), ()> {
        unsafe {
            match Apic::broadcast_invalidate_tlb() {
                true => Ok(()),
                false => Err(()),
            }
        }
    }
}

bitflags! {
    pub(super) struct PageAttributes: PageTableRepr {
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
pub(super) enum PageTableAvl {
    None = 0,
    Reserved = 1,
}

#[allow(dead_code)]
impl PageAttributes {
    const AVL_SHIFT: usize = 9;

    #[inline]
    pub const fn avl(self) -> PageTableAvl {
        unsafe { transmute((self.bits() & Self::AVL_MASK.bits()) >> Self::AVL_SHIFT) }
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
    pub const fn frame_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0 & Self::ADDRESS_BIT)
    }

    #[inline]
    pub fn attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0 & Self::NORMAL_ATTRIBUTE_BITS)
    }

    #[inline]
    pub fn set_frame_address(&mut self, pa: PhysicalAddress) {
        self.0 = (pa.as_u64() & Self::ADDRESS_BIT) | (self.0 & !Self::ADDRESS_BIT);
    }

    #[inline]
    pub fn set_attributes(&mut self, flags: PageAttributes) {
        self.0 = (self.0 & Self::ADDRESS_BIT) | (flags.bits() & !Self::ADDRESS_BIT);
    }
}

impl AddAssign<PageAttributes> for PageTableEntry {
    fn add_assign(&mut self, rhs: PageAttributes) {
        self.insert(rhs);
    }
}

impl SubAssign<PageAttributes> for PageTableEntry {
    fn sub_assign(&mut self, rhs: PageAttributes) {
        self.remove(rhs);
    }
}

impl BitOrAssign<PageAttributes> for PageTableEntry {
    fn bitor_assign(&mut self, rhs: PageAttributes) {
        self.insert(rhs);
    }
}

impl AddAssign<usize> for PageTableEntry {
    fn add_assign(&mut self, rhs: usize) {
        let pa = self.frame_address() + rhs;
        self.set_frame_address(pa);
    }
}

impl From<PhysicalAddress> for PageTableEntry {
    #[inline]
    fn from(value: PhysicalAddress) -> Self {
        Self(value.as_u64())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PageLevel {
    /// The official name is Page Table
    Level1,
    /// The official name is Page Directory Table
    Level2,
    /// The official name is Page Directory Pointer Table
    Level3,
    /// The official name is Page Map Level 4 Table
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
        PageManager::PAGE_KERNEL_PREFIX | (PageManager::PAGE_RECURSIVE << 39);
    pub const RECURSIVE_LV2: usize = Self::RECURSIVE_LV1 | (PageManager::PAGE_RECURSIVE << 30);
    pub const RECURSIVE_LV3: usize = Self::RECURSIVE_LV2 | (PageManager::PAGE_RECURSIVE << 21);
    pub const RECURSIVE_LV4: usize = Self::RECURSIVE_LV3 | (PageManager::PAGE_RECURSIVE << 12);

    #[inline]
    pub const fn parent(&self) -> Option<Self> {
        match *self {
            Self::Level1 => Some(Self::Level2),
            Self::Level2 => Some(Self::Level3),
            Self::Level3 => Some(Self::Level4),
            Self::Level4 => None,
        }
    }

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
