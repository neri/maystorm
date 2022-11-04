use crate::fs::{devfs::*, *};
use alloc::borrow::ToOwned;
use megstd::{fs::FileType, io::Result, Arc, String};

/// Null Device `/dev/null`
pub struct Null;

static NULL_INFO: DeviceCharacteristics = DeviceCharacteristics {
    file_type: FileType::CharDev,
    size: 0,
};

impl Null {
    pub fn new() -> Arc<dyn DeviceFileDriver> {
        Arc::new(Self)
    }
}

impl DeviceFileDriver for Null {
    fn name(&self) -> String {
        "null".to_owned()
    }

    fn info(&self) -> &DeviceCharacteristics {
        &NULL_INFO
    }

    fn open(&self) -> megstd::io::Result<Arc<dyn FsAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl FsAccessToken for Null {
    fn stat(&self) -> Option<FsRawMetaData> {
        Some(NULL_INFO.into())
    }

    fn read_data(&self, _offset: OffsetType, _buf: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Ok(0)
    }

    fn lseek(&self, _offset: OffsetType, _whence: Whence) -> Result<OffsetType> {
        Ok(0)
    }
}
