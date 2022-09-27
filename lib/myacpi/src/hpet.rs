use super::*;

/// High Precision Event Timers
#[repr(C, packed)]
#[allow(unused)]
pub struct Hpet {
    _hdr: AcpiHeader,
    block_id: u32,
    base_address: Gas,
    hpet_number: u8,
    clock_tick_unit: u16,
    attributes: u8,
}

unsafe impl AcpiTable for Hpet {
    const TABLE_ID: TableId = TableId::HPET;
}

impl Hpet {
    #[inline]
    pub const fn base_address(&self) -> u64 {
        self.base_address.address
    }
}
