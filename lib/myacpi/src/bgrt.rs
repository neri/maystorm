use super::*;

/// Boot Graphics Resource Table
#[repr(C, packed)]
pub struct Bgrt {
    _hdr: AcpiHeader,
    version: u16,
    status: u8,
    image_type: u8,
    image_address: u64,
    offset_x: u32,
    offset_y: u32,
}

unsafe impl AcpiTable for Bgrt {
    const TABLE_ID: TableId = TableId::BGRT;
}

impl Bgrt {
    #[inline]
    pub fn bitmap(&self) -> *const u8 {
        self.image_address as usize as *const u8
    }

    #[inline]
    pub const fn offset(&self) -> (usize, usize) {
        (self.offset_x as usize, self.offset_y as usize)
    }
}
