// Ram Disk

use super::fs::*;
use alloc::vec::Vec;
use core::ptr::*;

#[allow(dead_code)]
pub struct RamDisk {
    info: BlockDeviceInfo,
    base: *mut u8,
    dynamic: Option<Vec<u8>>,
}

impl RamDisk {
    /// Create a dynamic instance
    pub fn new(block_size: usize, total_blocks: usize) -> Option<Self> {
        let size = block_size * total_blocks;
        let mut blob = Vec::with_capacity(size);
        blob.resize(size, 0);
        Some(Self {
            info: BlockDeviceInfo::new(block_size, total_blocks, BlockDeviceFlag::empty()),
            base: &blob[0] as *const _ as *mut u8,
            dynamic: Some(blob),
        })
    }

    /// Create an instance from a static blob
    pub fn from_static(blob: &'static mut [u8], block_size: usize) -> Self {
        let total_blocks = blob.len() / block_size;
        Self {
            info: BlockDeviceInfo::new(block_size, total_blocks, BlockDeviceFlag::empty()),
            base: &blob[0] as *const _ as *mut _,
            dynamic: None,
        }
    }

    /// Make the volume read only
    pub fn make_readonly(&mut self) {
        self.info.flags().insert(BlockDeviceFlag::READ_ONLY)
    }
}

impl BlockDevice for RamDisk {
    fn info(&self) -> &BlockDeviceInfo {
        &self.info
    }

    fn super_block<'a>(&self) -> Option<&'a [u8]> {
        unsafe { slice_from_raw_parts(self.base, self.info.block_size()).as_ref() }
    }

    fn x_read(&self, index: usize, count: usize, buffer: &mut [u8]) -> BlockDeviceResult {
        let bytes = count * self.info.block_size();
        assert!(buffer.len() >= bytes);
        assert!(index + count <= self.info.total_blocks());

        unsafe {
            let src = self.base.add(index * self.info.block_size()) as *const _;
            buffer.as_mut_ptr().copy_from_nonoverlapping(src, bytes);
        }
        Ok(())
    }
}
