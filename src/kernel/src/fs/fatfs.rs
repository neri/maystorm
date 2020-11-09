// File Allocation Table Filesystem

use super::filesys::*;
use crate::io::error::*;
use crate::*;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::vec::*;
use bitflags::*;
use byteorder::*;
use core::mem::*;
use core::sync::atomic::*;

const DRIVER_NAME: &str = "fat";

pub struct FatFsDriver {
    _phantom: (),
}

impl FatFsDriver {
    pub(crate) fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }

    fn identify(dev: &'static Box<dyn BlockDevice>) -> Option<FatFs<'static>> {
        let boot_sector = match dev.super_block() {
            Some(bs) => bs,
            None => return None,
        };
        let ebpb: &ExtendedBpb = unsafe { transmute(&boot_sector[0] as *const _) };
        let bpb32: &Fat32Bpb = unsafe { transmute(&boot_sector[0] as *const _) };

        let bytes_per_sector = dev.info().block_size();
        if bytes_per_sector != ebpb.bpb.bytes_per_sector.into() {
            return None;
        }

        let record_shift = ebpb.bpb.sectors_per_cluster.trailing_zeros() as usize;

        let total_sectors = if ebpb.bpb.total_sectors16 != 0 {
            ebpb.bpb.total_sectors16 as u32
        } else {
            ebpb.bpb.total_sectors32
        };
        let sectors_per_fat = if ebpb.bpb.sectors_per_fat != 0 {
            ebpb.bpb.sectors_per_fat as u32
        } else {
            bpb32.sectors_per_fats
        };

        let fat_offset = ebpb.bpb.num_of_reserved_sectors as u32;
        let rootdir_offset = fat_offset + ebpb.bpb.num_of_fats as u32 * sectors_per_fat;
        let num_of_rootdir = ebpb.bpb.num_of_root_dirent as usize;
        let rootdir_sectors = ((num_of_rootdir * FatFs::SIZE_OF_DIRENT + bytes_per_sector - 1)
            / bytes_per_sector) as u32;
        let record_offset = rootdir_offset + rootdir_sectors;
        let total_records = (total_sectors - record_offset) >> record_shift;

        let (fs_type, rootdir_index) = if total_records < 4085 {
            (FatFsType::Fat12, FatFs::INODE_ROOTDIR)
        } else if total_records < 65525 {
            (FatFsType::Fat16, FatFs::INODE_ROOTDIR)
        } else if total_records < 268435445 {
            (FatFsType::Fat32, bpb32.index_of_root_directory as INodeType)
        } else {
            return None;
        };

        let volume_serial_number = match fs_type {
            FatFsType::Fat12 | FatFsType::Fat16 => {
                if ebpb.extended_boot_signature == FatFs::EXTENDED_BOOT_SIGNATURE {
                    ebpb.volume_serial_number
                } else {
                    0
                }
            }
            FatFsType::Fat32 => {
                if bpb32.extended_boot_signature == FatFs::EXTENDED_BOOT_SIGNATURE {
                    bpb32.volume_serial_number
                } else {
                    0
                }
            }
            _ => 0,
        };

        let info = FileSystemInfo {
            driver_name: DRIVER_NAME,
            fs_name: fs_type.to_str(),
            volume_serial_number,
            bytes_per_block: bytes_per_sector,
            bytes_per_record: bytes_per_sector << record_shift,
            total_blocks: total_sectors as u64,
            total_records: total_records as u64,
            free_records: 0,
        };

        Some(FatFs {
            block_device: dev,
            fs_type,
            info,
            special_chain: fs_type.special_chain(),
            record_shift,
            fat_offset,
            sectors_per_fat,
            rootdir_index,
            rootdir_offset,
            rootdir_sectors,
            num_of_rootdir,
            record_offset,
            next_allocation: 0,
            fat_cache: Vec::new(),
            rootdir_cache: Vec::new(),
            rootdir_entries: Vec::new(),
            inode_cache: BTreeMap::new(),
            next_file_id: AtomicU64::new(FatFs::INODE_FIRST),
        })
    }
}

impl FileSystemDriver for FatFsDriver {
    fn driver_name(&self) -> &str {
        DRIVER_NAME
    }

    fn mount(
        &self,
        dev: &'static Box<dyn BlockDevice>,
        options: &MountOption,
    ) -> Option<Box<dyn FileSystem>> {
        let _ = options;
        FatFsDriver::identify(dev).map(|mut fs| {
            fs.mount();
            fs.into_box()
        })
    }
}

#[allow(dead_code)]
struct FatFs<'a> {
    block_device: &'a Box<dyn BlockDevice>,
    fs_type: FatFsType,
    info: FileSystemInfo<'a>,
    special_chain: FatEntry,
    record_shift: usize,
    fat_offset: u32,
    sectors_per_fat: u32,
    rootdir_index: INodeType,
    rootdir_offset: u32,
    rootdir_sectors: u32,
    num_of_rootdir: usize,
    record_offset: u32,
    next_allocation: u32,
    fat_cache: Vec<u8>,
    rootdir_cache: Vec<u8>,
    rootdir_entries: Vec<INodeType>,
    inode_cache: BTreeMap<INodeType, FatFileDescriptor>,
    next_file_id: AtomicU64,
}

#[allow(dead_code)]
impl FatFs<'_> {
    const EXTENDED_BOOT_SIGNATURE: u8 = 0x29;
    const SIZE_OF_DIRENT: usize = 32;
    const INODE_ROOTDIR: INodeType = 1;
    const INODE_FIRST: INodeType = 2;

    fn next_file_id(&self) -> INodeType {
        self.next_file_id.fetch_add(1, Ordering::SeqCst)
    }

    fn mount(&mut self) {
        match self.fs_type {
            FatFsType::Fat12 | FatFsType::Fat16 => {
                let sectors_per_fat = self.sectors_per_fat as usize;
                self.fat_cache
                    .resize(sectors_per_fat * self.info.bytes_per_block, 0);
                self.block_device
                    .x_read(
                        self.fat_offset as usize,
                        sectors_per_fat,
                        &mut self.fat_cache,
                    )
                    .unwrap();

                self.info.free_records =
                    (0..self.info.total_records).fold(0 as FatEntry, |acc, x| {
                        if self.get_fat_entry(2 + x as FatEntry) == 0 {
                            acc + 1
                        } else {
                            acc
                        }
                    }) as u64;

                let rootdir_sectors = self.rootdir_sectors as usize;
                self.rootdir_cache
                    .resize(rootdir_sectors * self.info.bytes_per_block, 0);
                self.block_device
                    .x_read(
                        self.rootdir_offset as usize,
                        rootdir_sectors,
                        &mut self.rootdir_cache,
                    )
                    .unwrap();

                for i in 0..self.num_of_rootdir {
                    let head = i * Self::SIZE_OF_DIRENT;
                    let tail = head + Self::SIZE_OF_DIRENT;
                    let ent: &FatDirEnt =
                        unsafe { transmute(self.rootdir_cache[head..tail].as_ptr()) };
                    if ent.name[0] == 0 {
                        break;
                    }
                    if ent.name[0] == 0xE5 || ent.attr.contains(FatAttr::VOLUME_LABEL) {
                        continue;
                    }
                    let inode = self.next_file_id();
                    let fd = FatFileDescriptor::from_dirent(inode, Self::INODE_ROOTDIR, i, ent);
                    self.inode_cache.insert(inode, fd);
                    self.rootdir_entries.push(inode);
                }
            }
            _ => unimplemented!(),
        }
    }

    fn get_fat_entry(&self, index: FatEntry) -> FatEntry {
        match self.fs_type {
            FatFsType::Fat12 => {
                let offset = (index as usize * 3) / 2;
                let data = LE::read_u16(&self.fat_cache[offset..offset + 2]) as FatEntry;
                if (index & 1) == 0 {
                    (data & 0x0FFF) as FatEntry
                } else {
                    (data >> 4) as FatEntry
                }
            }
            FatFsType::Fat16 => {
                let offset = index as usize * 2;
                LE::read_u16(&self.fat_cache[offset..offset + 2]) as FatEntry
            }
            _ => todo!(),
        }
    }

    fn resolve_offset(&self, inode: INodeType, offset: usize) -> FatEntry {
        let fd = match self.inode_cache.get(&inode) {
            Some(fd) => fd,
            None => return 0,
        };
        let mut index = fd.fat_index;
        if index >= self.special_chain {
            return index;
        }
        for _ in 0..offset {
            if index == 0 {
                return 0;
            }
            if index >= self.special_chain {
                return index;
            }
            index = self.get_fat_entry(index);
        }
        index
    }

    fn read_record(
        &self,
        inode: INodeType,
        offset: usize,
        count: usize,
        buffer: &mut [u8],
    ) -> BlockDeviceResult {
        match inode {
            0 => return Err(Error::new(ErrorKind::InvalidInput)),
            Self::INODE_ROOTDIR => todo!(),
            _ => {
                let mut index = self.resolve_offset(inode, offset);
                for i in 0..count {
                    if index == 0 {
                        return Err(Error::new(ErrorKind::InvalidData));
                    }
                    if index >= self.special_chain {
                        return Err(Error::new(ErrorKind::UnexpectedEof));
                    }
                    let buffer = &mut buffer[i * self.info.bytes_per_record..];
                    let sector_index =
                        self.record_offset as usize + ((index as usize - 2) << self.record_shift);
                    match self
                        .block_device
                        .x_read(sector_index, 1 << self.record_shift, buffer)
                    {
                        Ok(_) => (),
                        Err(err) => return Err(err),
                    }
                    index = self.get_fat_entry(index);
                }
                Ok(())
            }
        }
    }

    fn read_dir_raw(&self, inode: INodeType, index: usize) -> Option<&FatFileDescriptor> {
        match inode {
            Self::INODE_ROOTDIR => self
                .rootdir_entries
                .get(index)
                .and_then(|key| self.inode_cache.get(key)),
            _ => unimplemented!(),
        }
    }
}

impl FatFs<'static> {
    fn into_box(self) -> Box<dyn FileSystem> {
        Box::new(self)
    }
}

impl FileSystem for FatFs<'_> {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn root_dir(&self) -> INodeType {
        self.rootdir_index
    }

    fn read_dir<'a>(&self, inode: INodeType, index: usize) -> Option<DirectoryEntry> {
        self.read_dir_raw(inode, index)
            .map(|ent| ent.into_directory())
    }

    fn stat(&self, inode: INodeType) -> Option<FileStat> {
        self.inode_cache.get(&inode).map(|fd| fd.into_stat(self))
    }

    fn x_read(&self, inode: INodeType, offset: usize, count: usize, buffer: &mut [u8]) -> usize {
        self.inode_cache
            .get(&inode)
            .map(|_fd| {
                self.read_record(inode, offset, count, buffer).unwrap();
                count
            })
            .unwrap_or(0)
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
enum FatFsType {
    Fat12,
    Fat16,
    Fat32,
    ExFat,
}

#[allow(dead_code)]
impl FatFsType {
    fn to_str(&self) -> &'static str {
        match self {
            FatFsType::Fat12 => "fat12",
            FatFsType::Fat16 => "fat16",
            FatFsType::Fat32 => "fat32",
            FatFsType::ExFat => "exfat",
        }
    }

    fn special_chain(&self) -> FatEntry {
        match self {
            FatFsType::Fat12 => 0x0000_0FF7,
            FatFsType::Fat16 => 0x0000_FFF7,
            FatFsType::Fat32 => 0x0FFF_FFF7,
            FatFsType::ExFat => 0xFFFF_FFF7,
        }
    }

    fn signature(&self) -> &'static [u8; 8] {
        match self {
            FatFsType::Fat12 => b"FAT12   ",
            FatFsType::Fat16 => b"FAT16   ",
            FatFsType::Fat32 => b"FAT32   ",
            FatFsType::ExFat => b"EXFAT   ",
        }
    }
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct Bpb {
    jumps: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    num_of_reserved_sectors: u16,
    num_of_fats: u8,
    num_of_root_dirent: u16,
    total_sectors16: u16,
    media_id: u8,
    sectors_per_fat: u16,
    sectors_per_track: u16,
    num_of_heads: u16,
    num_of_hidden_sectors: u32,
    total_sectors32: u32,
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct ExtendedBpb {
    bpb: Bpb,
    physical_drvive_number: u8,
    _reserved: u8,
    /// should be 0x29
    extended_boot_signature: u8,
    volume_serial_number: u32,
    /// may be "NO NAME    "
    volume_name: [u8; 11],
    /// should be "FAT12   " or "FAT16   "
    file_system_type: [u8; 8],
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct Fat32Bpb {
    bpb: Bpb,
    sectors_per_fats: u32,
    flags: u16,
    /// must be 0x0000
    version: u16,
    /// typically 2
    index_of_root_directory: u32,
    /// typically 1
    fsinfo: u16,
    /// typically 6
    copy_of_boot_sector: u16,
    /// may be [0xF6...]
    _reverded1: [u8; 12],
    physical_drvive_number: u8,
    /// may be 0xF6
    _reserved2: u8,
    /// should be 0x29
    extended_boot_signature: u8,
    volume_serial_number: u32,
    /// may be "NO NAME    "
    volume_name: [u8; 11],
    /// should be "FAT32   "
    file_system_type: [u8; 8],
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct ExFatBpb {
    jumps: [u8; 3],
    /// must be "EXFAT   "
    signature: [u8; 8],
    /// must be filled by 0
    _padding: [u8; 0x35],
    _nandakke: u64,
    total_sectors: u64,
    index_of_fat: u32,
    sectors_per_fats: u32,
    cluster_offset: u32,
    total_culsters: u32,
    index_of_root_directory: u32,
    _nandakke_: [u8; 1],
    flags: u16,
    sector_shift: u8,
    cluster_shift: u8,
    num_of_fats: u8,
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct FatDirEnt {
    name: [u8; 11],
    attr: FatAttr,
    nt_flags: u8,
    ctime_tenth: u8,
    ctime: u16,
    cdate: u16,
    adate: u16,
    index_hi: u16,
    mtime: u16,
    mdate: u16,
    index_lo: u16,
    size: u32,
}

bitflags! {
    pub struct FatAttr: u8 {
        const READ_ONLY     = 0b0000_0001;
        const HIDDEN        = 0b0000_0010;
        const SYSTEM        = 0b0000_0100;
        const VOLUME_LABEL  = 0b0000_1000;
        const DIRECTORY     = 0b0001_0000;
        const ARCHIVE       = 0b0010_0000;

        const LFN_ENTRY     = 0b0000_1111;
    }
}

impl FatDirEnt {
    /// Converts the file name string from FAT's 8.3 format to a filename string
    fn name(&self) -> String {
        let mut vec: Vec<u8> = Vec::with_capacity(12);
        let base_name = &self.name[0..8];
        let ext_name = &self.name[8..11];

        let mut base_name_len = 8;
        for i in (0..8).rev() {
            if base_name[i] > 0x20 {
                base_name_len = i + 1;
                break;
            }
        }
        let base_name = &base_name[..base_name_len];
        for c in base_name.iter() {
            vec.push(Self::convert_name_char(*c));
        }

        let mut ext_name_len = 0;
        for i in (0..3).rev() {
            if ext_name[i] > 0x20 {
                ext_name_len = i + 1;
                break;
            }
        }
        if ext_name_len > 0 {
            vec.push(0x2E);
            let ext_name = &ext_name[..ext_name_len];
            for c in ext_name.iter() {
                vec.push(Self::convert_name_char(*c));
            }
        }

        String::from_utf8(vec).unwrap()
    }

    fn convert_name_char(c: u8) -> u8 {
        if c >= 0x41 && c <= 0x5A {
            c | 0x20
        } else if c >= 0x20 && c <= 0x7F {
            c
        } else {
            0x5F
        }
    }
}

type FatEntry = u32;

#[allow(dead_code)]
struct FatFileDescriptor {
    inode: INodeType,
    name: String,
    dir_node: INodeType,
    dir_index: usize,
    fat_index: FatEntry,
    file_size: usize,
}

impl FatFileDescriptor {
    fn from_dirent(
        inode: INodeType,
        dir_node: INodeType,
        dir_index: usize,
        ent: &FatDirEnt,
    ) -> Self {
        Self {
            inode,
            name: ent.name(),
            dir_node,
            dir_index,
            fat_index: ent.index_lo as FatEntry,
            file_size: ent.size as usize,
        }
    }

    fn into_directory(&self) -> DirectoryEntry {
        DirectoryEntry::new(self.name.clone(), self.inode)
    }

    fn into_stat(&self, fs: &FatFs) -> FileStat {
        let file_size = self.file_size;
        let block_size = fs.info.bytes_per_record;
        let blocks = (file_size + block_size - 1) / block_size;
        FileStat {
            inode: self.inode,
            file_size,
            block_size,
            blocks,
        }
    }
}
