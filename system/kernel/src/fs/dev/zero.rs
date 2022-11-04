use crate::fs::{devfs::*, *};
use alloc::borrow::ToOwned;
use megstd::{fs::FileType, io::Result, Arc, String};

/// Zero Device `/dev/zero`
pub struct Zero;

static ZERO_INFO: DeviceCharacteristics = DeviceCharacteristics {
    file_type: FileType::CharDev,
    size: 0,
};

impl Zero {
    pub fn new() -> Arc<dyn DeviceFileDriver> {
        Arc::new(Self)
    }
}

impl DeviceFileDriver for Zero {
    fn name(&self) -> String {
        "zero".to_owned()
    }

    fn info(&self) -> &DeviceCharacteristics {
        &ZERO_INFO
    }

    fn open(&self) -> megstd::io::Result<Arc<dyn FsAccessToken>> {
        Ok(Arc::new(Self))
    }
}

impl FsAccessToken for Zero {
    fn stat(&self) -> Option<FsRawMetaData> {
        Some(ZERO_INFO.into())
    }

    fn read_data(&self, _offset: OffsetType, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write_data(&self, _offset: OffsetType, _buf: &[u8]) -> Result<usize> {
        Ok(0)
    }

    fn lseek(&self, _offset: OffsetType, _whence: Whence) -> Result<OffsetType> {
        Ok(0)
    }
}
