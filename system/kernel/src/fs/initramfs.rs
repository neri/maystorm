use super::*;
use crate::*;
use alloc::{
    borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec,
};
use byteorder::*;
use core::{
    intrinsics::copy_nonoverlapping,
    ptr::slice_from_raw_parts_mut,
    sync::atomic::{AtomicUsize, Ordering},
};
use megstd::{
    fs::FileType,
    io::{ErrorKind, Result},
};

/// Minimal Initial Ram Filesystem
pub struct InitRamfs {
    blob: Box<[u8]>,
    inodes: BTreeMap<INodeType, ThisFsInodeEntry>,
    root: INodeType,
    next_inode: AtomicUsize,
}

impl InitRamfs {
    const MAGIC_CURRENT: u32 = 0x0001beef;
    const SIZE_OF_RAW_DIR: usize = 32;
    const OFFSET_DATA: usize = 16;

    #[inline]
    pub unsafe fn from_static(base: *mut u8, len: usize) -> Option<Arc<dyn FsDriver>> {
        Self::new(Box::from_raw(slice_from_raw_parts_mut(base, len)))
    }

    fn new(blob: Box<[u8]>) -> Option<Arc<dyn FsDriver>> {
        if Self::MAGIC_CURRENT != LE::read_u32(&blob[0..4]) {
            return None;
        }

        let root = unsafe { INodeType::new_unchecked(2) };
        let mut fs = Self {
            blob,
            inodes: BTreeMap::new(),
            root,
            next_inode: AtomicUsize::new(root.get() as usize),
        };

        let dir_off = LE::read_u32(&fs.blob[4..8]) as usize - Self::OFFSET_DATA;
        let dir_size = LE::read_u32(&fs.blob[8..12]) as usize * Self::SIZE_OF_RAW_DIR;
        let mut root_dir = ThisFsInodeEntry {
            file_type: FileType::Dir,
            inode: root,
            offset: dir_off,
            size: dir_size,
            children: Vec::new(),
        };
        fs.parse_dir(&mut root_dir);
        fs.inodes.insert(root, root_dir);

        Some(Arc::new(fs) as Arc<dyn FsDriver>)
    }

    #[inline]
    fn next_inode(&self) -> INodeType {
        let v = 1 + self.next_inode.fetch_add(1, Ordering::SeqCst);
        INodeType::new(v as u64).unwrap()
    }

    fn parse_dir(&mut self, dir: &mut ThisFsInodeEntry) {
        let dir_off = dir.offset + Self::OFFSET_DATA;
        let n_dirent = dir.size / Self::SIZE_OF_RAW_DIR;

        for index in 0..n_dirent {
            let dir_offset = dir_off + index * Self::SIZE_OF_RAW_DIR;
            let lead_byte = self.blob[dir_offset];
            let is_dir = (lead_byte & 0x80) != 0;
            let name_len = 15 & lead_byte as usize;
            let name =
                String::from_utf8(self.blob[dir_offset + 1..dir_offset + name_len + 1].to_owned())
                    .unwrap_or("#NAME?".to_owned());
            let inode = self.next_inode();
            let dir_ent = ThisFsDirEntry {
                inode,
                name: name.to_owned(),
            };
            let mut entry = ThisFsInodeEntry {
                file_type: if is_dir {
                    FileType::Dir
                } else {
                    FileType::File
                },
                inode,
                offset: LE::read_u32(&self.blob[dir_offset + 0x18..dir_offset + 0x1C]) as usize,
                size: LE::read_u32(&self.blob[dir_offset + 0x1C..dir_offset + 0x20]) as usize,
                children: Vec::new(),
            };
            if is_dir {
                self.parse_dir(&mut entry);
            }

            self.inodes.insert(inode, entry);
            dir.children.push(dir_ent);
        }
    }

    #[inline]
    fn get_file(&self, inode: INodeType) -> Option<&ThisFsInodeEntry> {
        self.inodes.get(&inode)
    }
}

impl FsDriver for InitRamfs {
    fn device_name(&self) -> String {
        "initramfs".to_owned()
    }

    fn description(&self) -> String {
        "".to_owned()
    }

    fn root_dir(&self) -> INodeType {
        self.root
    }

    fn read_dir(&self, dir: INodeType, index: usize) -> Option<FsRawDirEntry> {
        let dir_ent = match self.get_file(dir).and_then(|v| v.children.get(index)) {
            Some(v) => v,
            None => return None,
        };
        let file = match self.get_file(dir_ent.inode) {
            Some(v) => v,
            None => return None,
        };
        Some(FsRawDirEntry::new(
            dir_ent.inode(),
            dir_ent.name(),
            file.into(),
        ))
    }

    fn find_file(&self, dir: INodeType, lpc: &str) -> Result<INodeType> {
        self.get_file(dir)
            .and_then(|v| v.children.iter().find(|v| v.name() == lpc))
            .map(|v| v.inode())
            .ok_or(ErrorKind::NotFound.into())
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
}

#[allow(dead_code)]
struct ThisFsInodeEntry {
    file_type: FileType,
    inode: INodeType,
    offset: usize,
    size: usize,
    children: Vec<ThisFsDirEntry>,
}

impl From<&ThisFsInodeEntry> for FsRawMetaData {
    fn from(src: &ThisFsInodeEntry) -> Self {
        Self::new(src.file_type, src.size as i64)
    }
}

struct ThisFsAccessToken {
    fs: Arc<InitRamfs>,
    inode: INodeType,
}

impl FsAccessToken for ThisFsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData> {
        self.fs.stat(self.inode)
    }

    fn read_data(&self, offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;
        let size_left = dir_ent.size as OffsetType - offset;
        let count = OffsetType::min(size_left, buf.len() as OffsetType) as usize;
        unsafe {
            let src = (&fs.blob[0] as *const _ as usize
                + InitRamfs::OFFSET_DATA
                + dir_ent.offset
                + offset as usize) as *const u8;
            let dst = &mut buf[0] as *mut _;
            copy_nonoverlapping(src, dst, count);
        }
        Ok(count)
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Err(ErrorKind::ReadOnlyFilesystem.into())
    }
}
