use super::*;

/// Fixed ACPI Description Table
#[repr(C, packed)]
#[allow(unused)]
pub struct Fadt {
    _hdr: AcpiHeader,
    // TODO:
}

unsafe impl AcpiTable for Fadt {
    const TABLE_ID: TableId = TableId::FADT;
}

impl Fadt {
    // TODO:
}
