// use crate::*;
use super::*;
use crate::sync::Mutex;
use core::sync::atomic::{AtomicUsize, Ordering};
use megstd::{
    fs::FileType,
    io::{ErrorKind, Result},
    Arc, BTreeMap, String, ToOwned, Vec,
};

const ROOT_INODE: INodeType = unsafe { INodeType::new_unchecked(2) };
// const BLOCK_SIZE: usize = 4096;
const FILE_SIZE_MAX: usize = i32::MAX as usize;

pub struct RamFs {
    inodes: Mutex<BTreeMap<INodeType, Arc<ThisFsInodeEntry>>>,
    next_inode: AtomicUsize,
}

impl RamFs {
    pub fn new() -> Arc<dyn FsDriver> {
        let fs = Self {
            inodes: Mutex::new(BTreeMap::new()),
            next_inode: AtomicUsize::new(ROOT_INODE.get() as usize),
        };

        let root_dir = Arc::new(ThisFsInodeEntry {
            inode: ROOT_INODE,
            file_type: ThisFsFileType::Directory(Mutex::new(Vec::new())),
        });
        fs.inodes.lock().unwrap().insert(ROOT_INODE, root_dir);

        Arc::new(fs) as Arc<dyn FsDriver>
    }

    fn make_node(self: Arc<Self>, dir: INodeType, name: &str, is_dir: bool) -> Result<INodeType> {
        let dir = self.get_file(dir).ok_or(ErrorKind::NotFound)?;
        let mut dir = match dir.file_type {
            ThisFsFileType::File(_) => return Err(ErrorKind::NotADirectory.into()),
            ThisFsFileType::Directory(ref v) => v.lock().unwrap(),
        };

        if dir
            .iter()
            .find(|v| Self::compare_name(v.name(), name))
            .is_some()
        {
            return Err(ErrorKind::AlreadyExists.into());
        }

        let content = if is_dir {
            ThisFsFileType::Directory(Mutex::new(Vec::new()))
        } else {
            ThisFsFileType::File(ThisFsContent::new())
        };
        let inode = self.next_inode();

        self.inodes.lock().unwrap().insert(
            inode,
            Arc::new(ThisFsInodeEntry {
                inode,
                file_type: content,
            }),
        );

        dir.push(ThisFsDirEntry {
            inode,
            name: name.to_owned(),
        });

        Ok(inode)
    }

    #[inline]
    #[allow(dead_code)]
    fn next_inode(&self) -> INodeType {
        let v = 1 + self.next_inode.fetch_add(1, Ordering::SeqCst);
        INodeType::new(v as u64).unwrap()
    }

    #[inline]
    fn get_file(&self, inode: INodeType) -> Option<Arc<ThisFsInodeEntry>> {
        self.inodes.lock().unwrap().get(&inode).map(|v| v.clone())
    }

    #[inline]
    fn compare_name(lhs: &str, rhs: &str) -> bool {
        lhs == rhs
    }
}

impl FsDriver for RamFs {
    fn device_name(&self) -> String {
        "ramfs".to_owned()
    }

    fn description(&self) -> String {
        "".to_owned()
    }

    fn root_dir(&self) -> INodeType {
        ROOT_INODE
    }

    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry> {
        let Some(dir) = self.get_file(dir) else {
            return None
        };
        let Some(dir_ent) = dir.nth_child(index) else {
            return None
        };
        let Some(file) = self.get_file(dir_ent.inode) else {
            return None
        };

        Some(FsRawDirEntry::new(
            dir_ent.inode(),
            dir_ent.name(),
            file.into(),
        ))
    }

    fn lookup(&self, dir: INodeType, lpc: &str) -> Result<INodeType> {
        let Some(dir) = self.get_file(dir) else {
            return Err(ErrorKind::NotFound.into())
        };

        dir.find_child(lpc).ok_or(ErrorKind::NotFound.into())
    }

    fn open(self: Arc<Self>, inode: INodeType) -> Result<Arc<dyn FsAccessToken>> {
        Ok(Arc::new(ThisFsAccessToken {
            fs: self.clone(),
            inode,
        }))
    }

    fn stat(&self, inode: INodeType) -> Option<FsRawMetaData> {
        self.get_file(inode).map(|v| v.into())
    }

    fn mkdir(self: Arc<Self>, dir: INodeType, name: &str) -> Result<()> {
        self.make_node(dir, name, true).map(|_| ())
    }

    fn creat(self: Arc<Self>, dir: INodeType, name: &str) -> Result<Arc<dyn FsAccessToken>> {
        self.clone().make_node(dir, name, false).map(|inode| {
            Arc::new(ThisFsAccessToken {
                fs: self.clone(),
                inode,
            }) as Arc<dyn FsAccessToken>
        })
    }

    fn unlink(&self, dir: INodeType, _name: &str) -> Result<()> {
        let _dir = self.get_file(dir).ok_or(ErrorKind::NotFound)?;

        Err(ErrorKind::PermissionDenied.into())
    }
}

struct ThisFsDirEntry {
    inode: INodeType,
    name: String,
}

impl ThisFsDirEntry {
    #[inline]
    pub const fn inode(&self) -> INodeType {
        self.inode
    }

    #[inline]
    pub fn name<'a>(&'a self) -> &'a str {
        self.name.as_str()
    }

    #[inline]
    pub fn clone(&self) -> Self {
        Self {
            inode: self.inode,
            name: self.name.clone(),
        }
    }
}

#[allow(dead_code)]
struct ThisFsInodeEntry {
    inode: INodeType,
    file_type: ThisFsFileType,
}

impl ThisFsInodeEntry {
    #[inline]
    pub fn file_type(&self) -> FileType {
        match self.file_type {
            ThisFsFileType::File(_) => FileType::File,
            ThisFsFileType::Directory(_) => FileType::Dir,
        }
    }

    #[inline]
    pub fn file_size(&self) -> usize {
        match self.file_type {
            ThisFsFileType::File(ref v) => v.estimated_size(),
            ThisFsFileType::Directory(ref v) => v.lock().unwrap().len(),
        }
    }

    #[inline]
    pub fn find_child(&self, name: &str) -> Option<INodeType> {
        match self.file_type {
            ThisFsFileType::File(_) => None,
            ThisFsFileType::Directory(ref dir) => dir
                .lock()
                .unwrap()
                .iter()
                .find(|v| RamFs::compare_name(v.name(), name))
                .map(|v| v.inode),
        }
    }

    #[inline]
    pub fn nth_child(&self, index: usize) -> Option<ThisFsDirEntry> {
        match self.file_type {
            ThisFsFileType::File(_) => None,
            ThisFsFileType::Directory(ref dir) => dir.lock().unwrap().get(index).map(|v| v.clone()),
        }
    }
}

impl From<Arc<ThisFsInodeEntry>> for FsRawMetaData {
    fn from(src: Arc<ThisFsInodeEntry>) -> Self {
        Self::new(src.inode, src.file_type(), src.file_size() as i64)
    }
}

enum ThisFsFileType {
    File(ThisFsContent),
    Directory(Mutex<Vec<ThisFsDirEntry>>),
}

struct ThisFsContent {
    estimated_size: AtomicUsize,
    content: Mutex<Vec<u8>>,
}

impl ThisFsContent {
    #[inline]
    pub const fn new() -> Self {
        Self {
            estimated_size: AtomicUsize::new(0),
            content: Mutex::new(Vec::new()),
        }
    }

    #[inline]
    pub fn estimated_size(&self) -> usize {
        self.estimated_size.load(Ordering::Relaxed)
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let content = self.content.lock().unwrap();
        if offset >= content.len() {
            return Ok(0);
        }
        let count = usize::min(buf.len(), content.len() - offset);
        unsafe {
            buf.as_mut_ptr()
                .copy_from_nonoverlapping(content.as_ptr().add(offset), count);
        }

        Ok(count)
    }

    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut content = self.content.lock().unwrap();
        let new_size = offset + buf.len();
        if new_size > FILE_SIZE_MAX {
            return Err(ErrorKind::FilesystemQuotaExceeded.into());
        }
        if content.capacity() < new_size {
            let additional = new_size - content.len();
            match content.try_reserve(additional) {
                Ok(_) => (),
                Err(_) => return Err(ErrorKind::StorageFull.into()),
            }
        }
        if offset < content.len() {
            content.resize(offset, 0);
        }
        let count = buf.len();
        if count > 0 {
            unsafe {
                content
                    .as_mut_ptr()
                    .add(offset)
                    .copy_from_nonoverlapping(buf.as_ptr(), count);
                if content.len() < new_size {
                    content.set_len(new_size);
                    self.estimated_size.store(content.len(), Ordering::SeqCst);
                }
            }
        }

        Ok(count)
    }

    pub fn truncate(&self, length: OffsetType) -> Result<()> {
        let length = if length >= 0 {
            length as usize
        } else {
            return Err(ErrorKind::InvalidInput.into());
        };
        let mut content = self.content.lock().unwrap();
        if content.len() <= length {
            content.resize(length, 0);
            Ok(())
        } else {
            Err(ErrorKind::InvalidInput.into())
        }
    }
}

struct ThisFsAccessToken {
    fs: Arc<RamFs>,
    inode: INodeType,
}

impl FsAccessToken for ThisFsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData> {
        self.fs.stat(self.inode)
    }

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;
        match dir_ent.file_type {
            ThisFsFileType::File(ref content) => content.read(offset as usize, buf),
            ThisFsFileType::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }

    fn write_data(&self, offset: OffsetType, buf: &[u8]) -> Result<usize> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;
        match dir_ent.file_type {
            ThisFsFileType::File(ref content) => content.write(offset as usize, buf),
            ThisFsFileType::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }

    fn truncate(&self, length: OffsetType) -> Result<()> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;
        match dir_ent.file_type {
            ThisFsFileType::File(ref content) => content.truncate(length),
            ThisFsFileType::Directory(_) => return Err(ErrorKind::IsADirectory.into()),
        }
    }
}
