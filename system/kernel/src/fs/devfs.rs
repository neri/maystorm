//! Device Filesystem (expr)

use super::*;
use crate::*;
use alloc::{borrow::ToOwned, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use megstd::{
    fs::FileType,
    io::{ErrorKind, Result},
};

/// Device Filesystem
pub struct DevFs {
    inodes: BTreeMap<INodeType, ThisFsInodeEntry>,
    root: INodeType,
    next_inode: AtomicUsize,
}

impl DevFs {
    pub fn new() -> Arc<dyn FsDriver> {
        let root = unsafe { INodeType::new_unchecked(2) };
        let mut fs = Self {
            inodes: BTreeMap::new(),
            root,
            next_inode: AtomicUsize::new(root.get() as usize),
        };

        let mut root_dir = ThisFsInodeEntry {
            file_type: FileType::Dir,
            inode: root,
            size: 0,
            children: Vec::new(),
        };

        {
            let (inode, dir_ent, inode_ent) = fs.make_dev("null");
            fs.inodes.insert(inode, inode_ent);
            root_dir.children.push(dir_ent);
        }

        fs.inodes.insert(root, root_dir);

        Arc::new(fs) as Arc<dyn FsDriver>
    }

    #[inline]
    fn next_inode(&self) -> INodeType {
        let v = 1 + self.next_inode.fetch_add(1, Ordering::SeqCst);
        INodeType::new(v as u64).unwrap()
    }

    fn make_dev(&self, name: &str) -> (INodeType, ThisFsDirEntry, ThisFsInodeEntry) {
        let inode = self.next_inode();
        let dir_ent = ThisFsDirEntry {
            inode,
            name: name.to_owned(),
        };
        let inode_ent = ThisFsInodeEntry {
            file_type: FileType::CharDev,
            inode,
            size: 0,
            children: Vec::new(),
        };
        (inode, dir_ent, inode_ent)
    }

    #[inline]
    fn get_file(&self, inode: INodeType) -> Option<&ThisFsInodeEntry> {
        self.inodes.get(&inode)
    }
}

impl FsDriver for DevFs {
    fn device_name(&self) -> String {
        "devfs".to_owned()
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
    size: usize,
    children: Vec<ThisFsDirEntry>,
}

impl From<&ThisFsInodeEntry> for FsRawMetaData {
    fn from(src: &ThisFsInodeEntry) -> Self {
        Self::new(src.file_type, src.size as i64)
    }
}

struct ThisFsAccessToken {
    fs: Arc<DevFs>,
    inode: INodeType,
}

impl FsAccessToken for ThisFsAccessToken {
    fn stat(&self) -> Option<FsRawMetaData> {
        self.fs.stat(self.inode)
    }

    fn read_data(&self, _offset: OffsetType, _buf: &mut [u8]) -> Result<usize> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;

        Ok(dir_ent.size)
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        let fs = self.fs.as_ref();
        let dir_ent = fs.get_file(self.inode).ok_or(ErrorKind::NotFound)?;

        Ok(dir_ent.size)
    }
}

impl Drop for ThisFsAccessToken {
    fn drop(&mut self) {
        // TODO:
    }
}