// FileSystem

use super::initramfs::*;
use alloc::string::String;
use alloc::vec::Vec;
use core::num::{NonZeroU64, NonZeroUsize};
use megstd::io;

static mut FS: FileManager = FileManager::new();

pub type OffsetType = i64;
pub type INodeType = u64;
pub type NonZeroINodeType = NonZeroU64;

pub struct FileManager {
    initramfs: Option<InitRamfs>,
}

impl FileManager {
    const fn new() -> Self {
        Self { initramfs: None }
    }

    pub(crate) unsafe fn init(initrd_base: usize, initrd_size: usize) {
        let shared = Self::shared_mut();
        shared.initramfs = InitRamfs::from_static(initrd_base, initrd_size);
    }

    #[inline]
    fn shared_mut<'a>() -> &'a mut Self {
        unsafe { &mut FS }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &FS }
    }

    #[inline]
    pub fn read_dir(_path: &str) -> io::Result<FsRawReadDir> {
        Ok(FsRawReadDir::new())
    }

    pub fn open(path: &str) -> io::Result<FsRawFileControlBlock> {
        let shared = FileManager::shared();
        let fs = match shared.initramfs.as_ref() {
            Some(v) => v,
            None => return Err(io::ErrorKind::NotConnected.into()),
        };

        let lpc = path; // TODO: parse path
        let inode = match fs.find_file(lpc) {
            Some(v) => v,
            None => return Err(io::ErrorKind::NotFound.into()),
        };
        let stat = match fs.stat(inode) {
            Some(v) => v,
            None => return Err(io::ErrorKind::InvalidData.into()),
        };

        let fcb = FsRawFileControlBlock::new(inode, stat.len());

        Ok(fcb)
    }
}

pub struct FsRawReadDir {
    index: usize,
}

impl FsRawReadDir {
    fn new() -> Self {
        Self { index: 0 }
    }
}

impl Iterator for FsRawReadDir {
    type Item = FsRawDirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let shared = FileManager::shared();
        shared
            .initramfs
            .as_ref()
            .and_then(|v| v.read_dir(self.index))
            .map(|v| {
                self.index += 1;
                v
            })
    }
}

pub struct FsRawDirEntry {
    inode: NonZeroINodeType,
    name: String,
    metadata: Option<FsRawMetaData>,
}

impl FsRawDirEntry {
    pub const fn new(
        inode: NonZeroINodeType,
        name: String,
        metadata: Option<FsRawMetaData>,
    ) -> Self {
        Self {
            inode,
            name,
            metadata,
        }
    }

    #[inline]
    pub const fn inode(&self) -> NonZeroINodeType {
        self.inode
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn metadata(&self) -> Option<&FsRawMetaData> {
        self.metadata.as_ref()
    }

    #[inline]
    pub fn into_metadata(self) -> Option<FsRawMetaData> {
        self.metadata
    }
}

pub struct FsRawMetaData {
    len: OffsetType,
}

impl FsRawMetaData {
    pub const fn new(len: OffsetType) -> Self {
        Self { len }
    }

    pub const fn len(&self) -> OffsetType {
        self.len
    }
}

pub struct FsRawFileControlBlock {
    inode: Option<NonZeroINodeType>,
    file_pos: OffsetType,
    file_size: OffsetType,
}

impl FsRawFileControlBlock {
    #[inline]
    const fn new(inode: NonZeroINodeType, file_size: OffsetType) -> Self {
        Self {
            inode: Some(inode),
            file_pos: 0,
            file_size,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let shared = FileManager::shared();
        shared
            .initramfs
            .as_ref()
            .ok_or(io::ErrorKind::NotConnected.into())
            .and_then(|v| v.read_data(self.inode, self.file_pos, buf))
            .map(|v| {
                self.file_pos += v as OffsetType;
                v
            })
    }

    pub fn read_to_end(&mut self, vec: &mut Vec<u8>) -> io::Result<usize> {
        let size = (self.file_size - self.file_pos) as usize;
        vec.resize(size, 0);
        self.read(vec.as_mut_slice()).map(|v| {
            vec.resize(v, 0);
            v
        })
    }

    pub fn lseek(&mut self, offset: OffsetType, whence: Whence) -> OffsetType {
        match whence {
            Whence::SeekSet => self.file_pos = offset,
            Whence::SeekCur => self.file_pos = self.file_pos + offset,
            Whence::SeekEnd => self.file_pos = self.file_size + offset,
        }
        self.file_pos
    }

    pub fn stat(&self) -> Option<FsRawMetaData> {
        let shared = FileManager::shared();
        self.inode
            .and_then(|inode| shared.initramfs.as_ref().and_then(|v| v.stat(inode)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FsFileHandle(NonZeroUsize);

impl FsFileHandle {
    // TODO:
}

#[derive(Debug, Copy, Clone)]
pub enum Whence {
    SeekSet = 0,
    SeekCur,
    SeekEnd,
}

impl From<usize> for Whence {
    fn from(v: usize) -> Self {
        match v {
            1 => Self::SeekCur,
            2 => Self::SeekEnd,
            _ => Self::SeekSet,
        }
    }
}
