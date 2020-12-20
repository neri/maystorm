// Portable Executable & Common Object File Format
use bitflags::*;
use core::marker::PhantomData;
use core::mem::*;
use core::slice;

pub const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;
pub const EFI_TE_IMAGE_HEADER_SIGNATURE: u16 = 0x5A56;

pub type Rva = u32;

#[repr(C, packed)]
pub struct PeHeader64 {
    pub signature: PeSignature,
    pub coff: CoffHeader,
    pub optional: OptionalHeaderPe64,
}

impl PeHeader64 {
    pub fn is_valid(&self) -> bool {
        unsafe { self.signature == PeSignature::IMAGE_NT_SIGNATURE && self.optional.is_valid() }
    }

    pub const fn size(&self) -> usize {
        size_of::<PeSignature>() + size_of::<CoffHeader>() + self.coff.size_of_optional as usize
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PeSignature {
    #[allow(non_camel_case_types)]
    IMAGE_NT_SIGNATURE = 0x00004550,
}

#[repr(C, packed)]
pub struct CoffHeader {
    pub machine: ImageFileMachine,
    pub n_sections: u16,
    pub time_stamp: u32,
    pub ptr_to_coff_symtab: u32,
    pub n_coff_symbols: u32,
    pub size_of_optional: u16,
    pub flags: ImageFile,
}

#[allow(dead_code, non_camel_case_types)]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageFileMachine {
    I386 = 0x014C,
    IA64 = 0x0200,
    AMD64 = 0x8664,
    EBC = 0x0EBC,
    ARM = 0x01C0,
    THUMB = 0x01C2,
    ARMNT = 0x01C4,
    ARM64 = 0xAA64,
    RISCV32 = 0x5032,
    RISCV64 = 0x5064,
    RISCV128 = 0x5128,
}

bitflags! {
    pub struct ImageFile: u16 {
        const RELOCS_STRIPPED       = 0x0001;
        const EXECUTABLE_IMAGE      = 0x0002;
        const LINE_NUMS_STRIPPED    = 0x0004;
        const LOCAL_SYMS_STRIPPED   = 0x0008;
        const MINIMAL_OBJECT        = 0x0010;
        const UPDATE_OBJECT         = 0x0020;
        const _16BIT_MACHINE        = 0x0040;
        const BYTES_REVERSED_LO     = 0x0080;
        const _32BIT_MACHINE        = 0x0100;
        const DEBUG_STRIPPED        = 0x0200;
        const PATCH                 = 0x0400;
        const SYSTEM                = 0x1000;
        const DLL                   = 0x2000;
        const BYTES_REVERSED_HI     = 0x8000;
    }
}

#[repr(C, packed)]
pub struct OptionalHeaderPe64 {
    pub magic: Magic,
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: u32,
    pub size_of_data: u32,
    pub size_of_bss: u32,
    pub entry_point: Rva,
    pub base_of_code: Rva,
    pub image_base: u64,
    pub section_align: u32,
    pub file_align: u32,
    pub major_os_version: u16,
    pub minor_os_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsys_version: u16,
    pub minor_subsys_version: u16,
    pub win32_reserved: u32,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub checksum: u32,
    pub subsystem: ImageSubsystem,
    pub dll_flags: u16,
    pub size_of_stack_reserve: u64,
    pub size_of_stack_commit: u64,
    pub size_of_heap_reserve: u64,
    pub size_of_heap_commit: u64,
    pub loader_flags: u32,
    pub n_dir: u32,
    pub dir: [ImageDataDirectory; 16],
}

impl OptionalHeaderPe64 {
    pub fn is_valid(&self) -> bool {
        unsafe { self.magic == Magic::PE64 }
    }
}

#[allow(dead_code)]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Magic {
    PE32 = 0x010B,
    PE64 = 0x020B,
}

#[allow(dead_code, non_camel_case_types)]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageSubsystem {
    UNKNOWN = 0,
    NATIVE = 1,
    WINDOWS_GUI = 2,
    WINDOWS_CUI = 3,
    OS2_CUI = 5,
    POSIX_CUI = 7,
    WINDOWS_CE_GUI = 9,
    EFI_APPLICATION = 10,
    EFI_BOOT_SERVICE_DRIVER = 11,
    EFI_RUNTIME_DRIVER = 12,
    EFI_ROM = 13,
    XBOX = 14,
    WINDOWS_BOOT_APPLICATION = 16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ImageDataDirectory {
    pub rva: Rva,
    pub size: u32,
}

#[allow(dead_code, non_camel_case_types)]
#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd)]
pub enum ImageDirectoryEntry {
    ARCHITECTURE = 7,
    BASERELOC = 5,
    BOUND_IMPORT = 11,
    COM_DESCRIPTOR = 14,
    DEBUG = 6,
    DELAY_IMPORT = 13,
    EXCEPTION = 3,
    EXPORT = 0,
    GLOBALPTR = 8,
    IAT = 12,
    IMPORT = 1,
    LOAD_CONFIG = 10,
    RESOURCE = 2,
    SECURITY = 4,
    TLS = 9,
}

impl core::ops::Index<ImageDirectoryEntry> for [ImageDataDirectory] {
    type Output = ImageDataDirectory;
    fn index(&self, index: ImageDirectoryEntry) -> &Self::Output {
        &self[index as usize]
    }
}

#[repr(C, packed)]
pub struct SectionTable {
    pub name: [u8; 8],
    pub vsize: u32,
    pub rva: u32,
    pub size: u32,
    pub file_offset: u32,
    pub reloc_ptr: u32,
    pub lineno_ptr: u32,
    pub n_reloc: u16,
    pub n_lineno: u16,
    pub flags: ImageScn,
}

bitflags! {
    pub struct ImageScn: u32 {
        const TYPE_DUMMY                = 0x0000_0001;
        const TYPE_NO_LOAD              = 0x0000_0002;
        const TYPE_GROUPED              = 0x0000_0004;
        const TYPE_NO_PAD               = 0x0000_0008;
        const TYPE_COPY                 = 0x0000_0010;
        const CNT_CODE                  = 0x0000_0020;
        const CNT_INITIALIZED_DATA      = 0x0000_0040;
        const CNT_UNINITIALIZED_DATA    = 0x0000_0080;
        const LNK_OTHER                 = 0x0000_0100;
        const LNK_INFO                  = 0x0000_0200;
        const LNK_OVERLAY               = 0x0000_0400;
        const LNK_REMOVE                = 0x0000_0800;
        const LNK_COMDAT                = 0x0000_1000;
        const MEM_DISCARDABLE           = 0x0200_0000;
        const MEM_NOT_CACHED            = 0x0400_0000;
        const MEM_NOT_PAGED             = 0x0800_0000;
        const MEM_SHARED                = 0x1000_0000;
        const MEM_EXECUTE               = 0x2000_0000;
        const MEM_READ                  = 0x4000_0000;
        const MEM_WRITE                 = 0x8000_0000;
    }
}

#[allow(dead_code)]
impl ImageScn {
    const TYPE_REGULAR: Self = Self::empty();
}

pub struct BaseReloc<'a> {
    base: *const u8,
    size: usize,
    index: usize,
    _phantom: &'a PhantomData<()>,
}

impl BaseReloc<'_> {
    pub unsafe fn new(base: *const u8, size: usize) -> Self {
        Self {
            base,
            size,
            index: 0,
            _phantom: &PhantomData,
        }
    }
}

impl<'a> Iterator for BaseReloc<'a> {
    type Item = &'a BaseRelocBlock;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let result: &BaseRelocBlock = unsafe { transmute(self.base.add(self.index)) };
            self.index += result.size as usize;
            Some(result)
        } else {
            None
        }
    }
}

#[repr(C, packed)]
pub struct BaseRelocBlock {
    rva_base: Rva,
    size: u32,
    entries: [BaseRelocEntry; 1],
}

impl BaseRelocBlock {
    pub const fn count(&self) -> usize {
        (self.size as usize - 8) / 2
    }
    pub fn entry<'a>(&self, index: usize) -> &'a BaseRelocEntry {
        let array = unsafe { slice::from_raw_parts(&self.entries[0], self.count()) };
        &array[index]
    }

    pub fn into_iter<'a>(&'a self) -> impl Iterator<Item = (ImageRelBased, Rva)> + 'a {
        BaseRelocBlockIter::<'a> {
            repr: &self,
            index: 0,
            len: self.count(),
        }
    }
}

struct BaseRelocBlockIter<'a> {
    repr: &'a BaseRelocBlock,
    index: usize,
    len: usize,
}

impl<'a> Iterator for BaseRelocBlockIter<'a> {
    type Item = (ImageRelBased, Rva);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            let item: &BaseRelocEntry = self.repr.entry(self.index);
            self.index += 1;
            Some((item.reloc_type(), self.repr.rva_base + item.offset()))
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct BaseRelocEntry(u16);

impl BaseRelocEntry {
    pub const fn offset(&self) -> Rva {
        self.0 as Rva & 0xFFF
    }

    pub const fn reloc_type(&self) -> ImageRelBased {
        ImageRelBased(self.0 as usize >> 12)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ImageRelBased(pub usize);

#[allow(dead_code)]
impl ImageRelBased {
    pub const ABSOLUTE: Self = Self(0);
    pub const HIGH: Self = Self(1);
    pub const LOW: Self = Self(2);
    pub const HIGHLOW: Self = Self(3);
    pub const HIGHADJ: Self = Self(4);
    pub const MIPS_JMPADDR: Self = Self(5);
    pub const ARM_MOV32: Self = Self(5);
    pub const RISCV_HIGH20: Self = Self(5);
    pub const THUMB_MOV32: Self = Self(7);
    pub const RISCV_LOW12I: Self = Self(7);
    pub const RISCV_LOW12S: Self = Self(8);
    pub const MIPS_JMPADDR16: Self = Self(9);
    pub const DIR64: Self = Self(10);
}
