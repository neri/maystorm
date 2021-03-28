// Executable and Linking Format

pub type ElfHalf = u16;
pub type ElfWord = u32;
pub type ElfXWord = u64;
pub type Elf32Addr = u32;
pub type Elf32Off = u32;
pub type Elf64Addr = u64;
pub type Elf64Off = u64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf32Hdr {
    pub e_ident: [u8; Self::EI_NIDENT],
    pub e_type: ElfType,
    pub e_machine: Machine,
    pub e_version: ElfWord,
    pub e_entry: Elf32Addr,
    pub e_phoff: Elf32Off,
    pub e_shoff: Elf32Off,
    pub e_flags: ElfWord,
    pub e_ehsize: ElfHalf,
    pub e_phentsize: ElfHalf,
    pub e_phnum: ElfHalf,
    pub e_shentsize: ElfHalf,
    pub e_shnum: ElfHalf,
    pub e_shstrndx: ElfHalf,
}

impl Elf32Hdr {
    pub const EI_NIDENT: usize = 16;
    pub const MAGIC: [u8; 4] = *b"\x7FELF";

    pub fn is_valid(&self) -> bool {
        (self.e_ident[..4] == Self::MAGIC)
            && (self.e_ident[4] == 1)
            && (self.e_ident[5] == 1)
            && (self.e_ident[6] == 1)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Hdr {
    pub n_ident: [u8; 16],
    pub e_type: ElfType,
    pub e_machine: Machine,
    pub e_version: ElfWord,
    pub e_entry: Elf64Addr,
    pub e_phoff: Elf64Off,
    pub e_shoff: Elf64Off,
    pub e_flags: ElfWord,
    pub e_ehsize: ElfHalf,
    pub e_phentsize: ElfHalf,
    pub e_phnum: ElfHalf,
    pub e_shentsize: ElfHalf,
    pub e_shnum: ElfHalf,
    pub e_shstrndx: ElfHalf,
}

impl Elf64Hdr {
    pub const MAGIC: [u8; 4] = *b"\x7FELF";

    pub fn is_valid(&self) -> bool {
        (self.n_ident[..4] == Self::MAGIC)
            && (self.n_ident[4] == 2)
            && (self.n_ident[5] == 1)
            && (self.n_ident[6] == 1)
    }
}

#[repr(u16)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElfType {
    NONE = 0,
    REL = 1,
    EXEC = 2,
    DYN = 3,
    CORE = 4,
}

#[repr(u16)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Machine {
    None = 0,
    M32 = 1,
    SPARC = 2,
    _386 = 3,
    MIPS = 8,
    PowerPC = 0x14,
    Arm = 0x28,
    IA64 = 0x32,
    x86_64 = 0x3E,
    Arch64 = 0xB7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf32Phdr {
    pub p_type: ElfSegmentType,
    pub p_offset: Elf32Off,
    pub p_vaddr: Elf32Addr,
    pub p_paddr: Elf32Addr,
    pub p_filesz: ElfWord,
    pub p_memsz: ElfWord,
    pub p_flags: ElfWord,
    pub p_align: ElfWord,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: ElfSegmentType,
    pub p_flags: ElfWord,
    pub p_offset: Elf64Off,
    pub p_vaddr: Elf64Addr,
    pub p_paddr: Elf64Addr,
    pub p_filesz: ElfXWord,
    pub p_memsz: ElfXWord,
    pub p_align: ElfXWord,
}

#[repr(u32)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElfSegmentType {
    NULL = 0,
    LOAD = 1,
    DYBAMIC = 2,
    INTERP = 3,
    NOTE = 4,
    SHLIB = 5,
    PHDR = 6,
    TLS = 7,
    PT_LOOS = 0x6000_0000,
    PT_GNU_EH_FRAME = 0x6474e550,
    PT_GNU_STACK = 0x6474e551,
    PT_SUNW_UNWIND = 0x6464e550,
    PT_HIOS = 0x6FFF_FFFF,
    PT_LOPROC = 0x7000_0000,
    PT_HIPROC = 0x7FFF_FFFF,
}
