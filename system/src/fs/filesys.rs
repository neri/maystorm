use super::{devfs::DevFs, initramfs::*};
use crate::{
    sync::{RwLock, RwLockReadGuard},
    task::scheduler::Scheduler,
    *,
};
use alloc::{
    borrow::ToOwned, collections::BTreeMap, fmt, format, string::String, sync::Arc, vec::Vec,
};
use core::{fmt::Display, num::NonZeroU64};
use megstd::{
    fs::*,
    io::{ErrorKind, Read, Result, Write},
};

static FS: FileManager = FileManager::new();

pub type OffsetType = i64;

pub struct FileManager {
    mount_points: RwLock<BTreeMap<String, Arc<dyn FsDriver>>>,
}

unsafe impl Send for FileManager {}

unsafe impl Sync for FileManager {}

impl FileManager {
    pub const PATH_SEPARATOR: &'static str = "/";

    #[inline]
    const fn new() -> Self {
        Self {
            mount_points: RwLock::new(BTreeMap::new()),
        }
    }

    pub unsafe fn init(initrd_base: *mut u8, initrd_size: usize) {
        assert_call_once!();

        let shared = Self::shared();

        let mut mount_points = shared.mount_points.write().unwrap();
        let bootfs =
            InitRamfs::from_static(initrd_base, initrd_size).expect("Unable to access bootfs");
        mount_points.insert(Self::PATH_SEPARATOR.to_owned(), bootfs.clone());
        mount_points.insert("/dev/".to_owned(), DevFs::init());
        mount_points.insert("/boot/".to_owned(), bootfs.clone());
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        &FS
    }

    #[inline]
    fn _join_path(path_components: &Vec<&str>) -> String {
        format!(
            "{}{}",
            Self::PATH_SEPARATOR,
            path_components.join(Self::PATH_SEPARATOR)
        )
    }

    fn _canonical_path_components(base: &str, path: &str) -> Vec<String> {
        let path = if path.starts_with("/") {
            path.to_owned()
        } else {
            format!("{}{}{}", base, Self::PATH_SEPARATOR, path)
        };

        let mut path_components = Vec::new();
        for component in path.split("/") {
            if component.is_empty() || component == "." {
            } else if component == ".." {
                let _ = path_components.pop();
            } else {
                path_components.push(component);
            }
        }

        path_components.into_iter().map(|v| v.to_owned()).collect()
    }

    pub fn canonical_path_components(path: &str) -> Vec<String> {
        Self::_canonical_path_components(Scheduler::current_pid().cwd().as_str(), path)
    }

    pub fn canonical_path(path: &str) -> String {
        let path_components = Self::canonical_path_components(path);
        let path_components = path_components.iter().map(|v| v.as_str()).collect();
        Self::_join_path(&path_components)
    }

    fn resolv(path: &str) -> Result<(Arc<dyn FsDriver>, INodeType)> {
        let shared = FileManager::shared();
        let mount_points = shared.mount_points.read().unwrap();

        let fq_path = format!("{}{}", Self::canonical_path(path), Self::PATH_SEPARATOR);

        let mut prefixes = mount_points.keys().collect::<Vec<_>>();
        prefixes.sort();
        let prefix = prefixes
            .into_iter()
            .rev()
            .find(|&v| fq_path.starts_with(v))
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;
        let fs = mount_points
            .get(prefix)
            .map(|v| v.clone())
            .ok_or(megstd::io::Error::from(ErrorKind::NotFound))?;

        let resolved = &fq_path[prefix.len() - 1..];
        let mut dir = fs.root_dir();
        for pc in Self::_canonical_path_components(Self::PATH_SEPARATOR, resolved) {
            dir = fs.lookup(dir, pc.as_str())?;
        }
        Ok((fs, dir))
    }

    pub fn mkdir(_path: &str) -> Result<()> {
        Err(ErrorKind::Unsupported.into())
    }

    pub fn rmdir(_path: &str) -> Result<()> {
        Err(ErrorKind::Unsupported.into())
    }

    pub fn chdir(path: &str) -> Result<()> {
        let path_components = Self::canonical_path_components(path);
        let (fs, inode) = Self::resolv(path)?;
        let stat = fs.stat(inode).ok_or(ErrorKind::NotFound)?;
        if !stat.file_type().is_dir() {
            return Err(ErrorKind::NotADirectory.into());
        }
        let path_components = path_components.iter().map(|v| v.as_str()).collect();
        Scheduler::current_pid().set_cwd(Self::_join_path(&path_components).as_str());
        Ok(())
    }

    pub fn read_dir(path: &str) -> Result<impl Iterator<Item = FsRawDirEntry>> {
        let (fs, dir) = Self::resolv(&path)?;
        Ok(FsRawReadDir::new(fs, dir))
    }

    pub fn open(path: &str) -> Result<FsRawFileControlBlock> {
        let (fs, inode) = Self::resolv(path)?;

        let Some(stat) = fs.stat(inode) else {
            return Err(ErrorKind::NotFound.into())
        };
        if stat.file_type().is_dir() {
            return Err(ErrorKind::IsADirectory.into());
        }

        let access_token = fs.open(inode)?;
        let fcb = FsRawFileControlBlock::new(
            access_token,
            stat.file_type().is_char_device() || stat.file_type().is_block_device(),
        );

        Ok(fcb)
    }

    pub fn unlink(_path: &str) -> Result<()> {
        Err(ErrorKind::Unsupported.into())
    }

    pub fn stat(path: &str) -> Result<FsRawMetaData> {
        let (fs, inode) = Self::resolv(path)?;
        let stat = fs.stat(inode).ok_or(ErrorKind::NotFound)?;
        Ok(stat)
    }

    pub fn mount_points<'a>() -> RwLockReadGuard<'a, BTreeMap<String, Arc<dyn FsDriver>>> {
        let shared = FileManager::shared();
        shared.mount_points.read().unwrap()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct INodeType(NonZeroU64);

impl INodeType {
    #[inline]
    pub const unsafe fn new_unchecked(val: u64) -> Self {
        Self(NonZeroU64::new_unchecked(val))
    }

    #[inline]
    pub const fn new(val: u64) -> Option<Self> {
        match NonZeroU64::new(val) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn get(&self) -> u64 {
        self.0.get()
    }
}

impl Display for INodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

pub trait FsDriver {
    /// Device name if mounted on physical device, otherwise name of file system driver
    fn device_name(&self) -> String;
    /// Return mount options as string
    fn description(&self) -> String;
    /// Returns the inode of the root directory
    fn root_dir(&self) -> INodeType;
    /// Reads the specified directory
    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry>;
    /// Searches for a file system object with the specified file name in the specified directory.
    fn lookup(&self, dir: INodeType, name: &str) -> Result<INodeType>;
    /// Opens a file with the specified inode
    fn open(self: Arc<Self>, inode: INodeType) -> Result<Arc<dyn FsAccessToken>>;
    /// Obtains metadata for the specified inode
    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData>;

    fn unlink(&self, _dir: INodeType, _name: &str) -> Result<()> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }

    fn mkdir(&self, _dir: INodeType, _name: &str) -> Result<INodeType> {
        Err(ErrorKind::Unsupported.into())
    }

    fn rmdir(&self, _dir: INodeType, _name: &str) -> Result<()> {
        Err(ErrorKind::Unsupported.into())
    }
}

pub trait FsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData>;

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize>;

    fn write_data(&self, offset: OffsetType, buf: &[u8]) -> Result<usize>;

    fn lseek(&self, offset: OffsetType, whence: Whence) -> Result<OffsetType>;
}

pub struct FsRawReadDir {
    fs: Arc<dyn FsDriver>,
    dir: INodeType,
    index: usize,
}

impl FsRawReadDir {
    fn new(fs: Arc<dyn FsDriver>, dir: INodeType) -> Self {
        Self { fs, dir, index: 0 }
    }
}

impl Iterator for FsRawReadDir {
    type Item = FsRawDirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.fs.as_ref().read_dir(self.dir, self.index).map(|v| {
            self.index += 1;
            v
        })
    }
}

pub struct FsRawDirEntry {
    inode: INodeType,
    name: String,
    metadata: FsRawMetaData,
}

impl FsRawDirEntry {
    #[inline]
    pub fn new(inode: INodeType, name: &str, metadata: FsRawMetaData) -> Self {
        Self {
            inode,
            name: name.to_owned(),
            metadata,
        }
    }

    #[inline]
    pub const fn inode(&self) -> INodeType {
        self.inode
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn metadata(&self) -> &FsRawMetaData {
        &self.metadata
    }
}

pub struct FsRawMetaData {
    inode: INodeType,
    file_type: FileType,
    len: OffsetType,
}

impl FsRawMetaData {
    #[inline]
    pub const fn new(inode: INodeType, file_type: FileType, len: OffsetType) -> Self {
        Self {
            inode,
            file_type,
            len,
        }
    }

    #[inline]
    pub const fn inode(&self) -> INodeType {
        self.inode
    }

    #[inline]
    pub const fn file_type(&self) -> FileType {
        self.file_type
    }

    #[inline]
    pub const fn len(&self) -> OffsetType {
        self.len
    }
}

pub struct FsRawFileControlBlock {
    access_token: Arc<dyn FsAccessToken>,
    is_device: bool,
    file_pos: OffsetType,
}

impl FsRawFileControlBlock {
    #[inline]
    fn new(access_token: Arc<dyn FsAccessToken>, is_device: bool) -> Self {
        Self {
            access_token,
            is_device,
            file_pos: 0,
        }
    }

    pub fn lseek(&mut self, offset: OffsetType, whence: Whence) -> Result<OffsetType> {
        if self.is_device {
            self.access_token.lseek(offset, whence)
        } else {
            match whence {
                Whence::SeekSet => self.file_pos = offset,
                Whence::SeekCur => self.file_pos = self.file_pos + offset,
                Whence::SeekEnd => match self.access_token.stat() {
                    Some(stat) => self.file_pos = stat.len() + offset,
                    None => return Ok(0),
                },
            }
            Ok(self.file_pos)
        }
    }

    pub fn fstat(&self) -> Option<FsRawMetaData> {
        self.access_token.stat()
    }
}

impl Read for FsRawFileControlBlock {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.access_token.read_data(self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    fn read_to_end(&mut self, vec: &mut Vec<u8>) -> Result<usize> {
        const BUFFER_SIZE: usize = 0x10000;
        let mut buffer = Vec::new();
        buffer
            .try_reserve(BUFFER_SIZE)
            .map_err(|_| megstd::io::Error::from(ErrorKind::OutOfMemory))?;
        buffer.resize(BUFFER_SIZE, 0);

        let mut count_read = 0;
        loop {
            match self.read(buffer.as_mut_slice()) {
                Ok(new_len) => {
                    if new_len == 0 {
                        return Ok(count_read);
                    }
                    if vec.try_reserve(new_len).is_err() {
                        return Err(ErrorKind::OutOfMemory.into());
                    }
                    if new_len < buffer.len() {
                        vec.extend_from_slice(&buffer[..new_len]);
                    } else {
                        vec.extend_from_slice(buffer.as_slice());
                    }
                    count_read += new_len;
                }
                Err(err) => match err.kind() {
                    ErrorKind::Interrupted => (),
                    _ => return Err(err),
                },
            }
        }
    }
}

impl Write for FsRawFileControlBlock {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.access_token.write_data(self.file_pos, buf).map(|v| {
            self.file_pos += v as OffsetType;
            v
        })
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
    }
}

impl fmt::Write for FsRawFileControlBlock {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes())
            .map(|_| ())
            .map_err(|_| core::fmt::Error)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Whence {
    SeekSet = 0,
    SeekCur,
    SeekEnd,
}

impl Default for Whence {
    #[inline]
    fn default() -> Self {
        Self::SeekSet
    }
}

impl TryFrom<usize> for Whence {
    type Error = ();

    fn try_from(value: usize) -> core::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::SeekSet),
            1 => Ok(Self::SeekCur),
            2 => Ok(Self::SeekEnd),
            _ => Err(()),
        }
    }
}
