// Virtual FileSystem

use super::fatfs::*;
use super::ramdisk::*;
use crate::io::error::*;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use bitflags::*;
use core::ptr::*;

static mut FS: Fs = Fs::new();

pub type INodeType = u64;

pub struct Fs {
    block_devices: Vec<Box<dyn BlockDevice>>,
    fs_drivers: Vec<Box<dyn FileSystemDriver>>,
    volumes: Vec<Box<dyn FileSystem>>,
}

impl Fs {
    const fn new() -> Self {
        Self {
            block_devices: Vec::new(),
            fs_drivers: Vec::new(),
            volumes: Vec::new(),
        }
    }

    pub(crate) fn init(initrd_base: usize, initrd_size: usize) {
        let shared = Self::shared();

        shared.fs_drivers.push(FatFsDriver::new());

        let blob = slice_from_raw_parts_mut(initrd_base as *mut _, initrd_size);
        shared.block_devices.push(Box::new(RamDisk::from_static(
            unsafe { blob.as_mut().unwrap() },
            BlockDeviceInfo::DEFAULT_BLOCK_SIZE,
        )));

        let options = MountOption { cmdline: "" };
        for dev in &shared.block_devices {
            'jump: loop {
                for fs in &shared.fs_drivers {
                    if let Some(volume) = fs.mount(dev, &options) {
                        shared.volumes.push(volume);
                        break 'jump;
                    }
                }
            }
        }
    }

    fn shared() -> &'static mut Self {
        unsafe { &mut FS }
    }

    pub fn list_of_volumes<'a>() -> &'a [Box<dyn FileSystem>] {
        let shared = Self::shared();
        shared.volumes.as_slice()
    }

    fn read_dir_iter<'a>(
        fs: &'a Box<dyn FileSystem>,
        inode: INodeType,
    ) -> impl Iterator<Item = DirectoryEntry> + 'a {
        FsReadDirIter::<'a> {
            fs,
            inode,
            index: 0,
        }
    }

    pub fn find_file(name: &str) -> Option<(&'static Box<dyn FileSystem>, INodeType)> {
        let shared = Self::shared();
        for fs in &shared.volumes {
            let inode = fs.root_dir();
            for file in fs.read_dir_iter(inode) {
                if file.name() == name {
                    return Some((fs, file.inode()));
                }
            }
        }
        None
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub struct MountOption<'a> {
    pub cmdline: &'a str,
}

pub trait FileSystemDriver {
    fn driver_name(&self) -> &str;

    fn mount(
        &self,
        dev: &'static Box<dyn BlockDevice>,
        options: &MountOption,
    ) -> Option<Box<dyn FileSystem>>;
}

pub trait FileSystem {
    /// Get Filesystem information
    fn info(&self) -> &FileSystemInfo;

    /// Get the inode of the root directory
    fn root_dir(&self) -> INodeType;

    /// Read directory
    fn read_dir(&self, inode: INodeType, index: usize) -> Option<DirectoryEntry>;

    /// stat
    fn stat(&self, inode: INodeType) -> Option<FileStat>;

    /// read file contents
    fn x_read(&self, inode: INodeType, offset: usize, count: usize, buffer: &mut [u8]) -> usize;
}

impl dyn FileSystem {
    #[inline]
    pub fn read_dir_iter<'a>(
        self: &'a Box<dyn FileSystem>,
        inode: INodeType,
    ) -> impl Iterator<Item = DirectoryEntry> + 'a {
        Fs::read_dir_iter(self, inode)
    }
}

struct FsReadDirIter<'a> {
    fs: &'a Box<dyn FileSystem>,
    inode: INodeType,
    index: usize,
}

impl Iterator for FsReadDirIter<'_> {
    type Item = DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.fs.read_dir(self.inode, self.index).map(|dir| {
            self.index += 1;
            dir
        })
    }
}

pub struct DirectoryEntry {
    name: String,
    inode: INodeType,
}

impl DirectoryEntry {
    pub const fn new(name: String, inode: INodeType) -> Self {
        Self { name, inode }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn inode(&self) -> INodeType {
        self.inode
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FileStat {
    pub inode: INodeType,
    pub file_size: usize,
    pub block_size: usize,
    pub blocks: usize,
}

pub struct BlockDeviceInfo {
    block_size: usize,
    total_blocks: usize,
    flags: BlockDeviceFlag,
}

impl BlockDeviceInfo {
    pub const DEFAULT_BLOCK_SIZE: usize = 512;

    pub const fn new(block_size: usize, total_blocks: usize, flags: BlockDeviceFlag) -> Self {
        Self {
            block_size,
            total_blocks,
            flags,
        }
    }

    pub const fn block_size(&self) -> usize {
        self.block_size
    }

    pub const fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    pub const fn is_readonly(&self) -> bool {
        self.flags.contains(BlockDeviceFlag::READ_ONLY)
    }

    pub fn flags(&mut self) -> &mut BlockDeviceFlag {
        &mut self.flags
    }
}

bitflags! {
    pub struct BlockDeviceFlag: usize {
        const READ_ONLY = 0b0000_0001;
        const REMOVABLE = 0b0000_0010;
    }
}

pub type BlockDeviceResult = Result<(), Error>;

pub trait BlockDevice {
    /// Get information of this block device
    fn info(&self) -> &BlockDeviceInfo;

    /// Reads the super block from the cache
    fn super_block<'a>(&self) -> Option<&'a [u8]>;

    /// Reads synchronously from the device (experimental)
    fn x_read(&self, index: usize, count: usize, buffer: &mut [u8]) -> BlockDeviceResult;

    // fn open(&self)
    // fn close(&self)
    // fn strategy(&self)
    // fn intr(&self)
}

pub struct FileSystemInfo<'a> {
    pub driver_name: &'a str,
    pub fs_name: &'a str,
    pub volume_serial_number: u32,

    pub bytes_per_block: usize,
    pub bytes_per_record: usize,
    pub total_blocks: u64,
    pub total_records: u64,
    pub free_records: u64,
}
