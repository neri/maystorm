use super::*;

/// Fixed ACPI Description Table
#[repr(C, packed)]
#[allow(unused)]
pub struct Fadt {
    hdr: AcpiHeader,
    firmware_ctrl: u32,
    dsdt: u32,
    _reserved1: u8,
    preferred_pm_profile: u8,
    sci_int: u16,
    smi_cmd: u32,
    acpi_enable: u8,
    acpi_disable: u8,
    s4bios_req: u8,
    pstate_cnt: u8,
    pm1a_evt_blk: u32,
    pm1b_evt_blk: u32,
    pm1a_cnt_blk: u32,
    pm1b_cnt_blk: u32,
    pm2_cnt_blk: u32,
    pm_tmr_blk: u32,
    gpe0_blk: u32,
    gpe1_blk: u32,
    pm1_evt_len: u8,
    pm1_cnt_len: u8,
    pm2_cnt_len: u8,
    pm_tmr_len: u8,
    gpe0_blk_len: u8,
    gpe1_blk_len: u8,
    gpe1_base: u8,
    cst_cnt: u8,
    p_lvl2_lat: u16,
    p_lvl3_lat: u16,
    flush_size: u16,
    flush_stride: u16,
    duty_offset: u8,
    duty_width: u8,
    day_alrm: u8,
    mon_alrm: u8,
    century: u8,
    iapc_boot_arch: u16,
    _reserved2: u8,
    flags: u32,
    reset_reg: Gas,
    reset_value: u8,
    arm_boot_arch: u16,
    fadt_minor_version: u8,
    x_firmware_ctrl: u64,
    x_dsdt: u64,
    x_pm1a_evt_blk: Gas,
    x_pm1b_evt_blk: Gas,
    x_pm1a_cnt_blk: Gas,
    x_pm1b_cnt_blk: Gas,
    x_pm2_cnt_blk: Gas,
    x_pm_tmr_blk: Gas,
    x_gpe0_blk: Gas,
    x_gpe1_blk: Gas,
    sleep_control_reg: Gas,
    sleep_status_reg: Gas,
    hyper_visor_vendor_identity: u64,
}

unsafe impl AcpiTable for Fadt {
    const TABLE_ID: TableId = TableId::FADT;
}

impl Fadt {
    #[inline]
    pub const fn sci_int(&self) -> u16 {
        self.sci_int
    }

    #[inline]
    pub const fn acpi_enable(&self) -> (u32, u8, u8) {
        (self.smi_cmd, self.acpi_enable, self.acpi_disable)
    }

    #[inline]
    fn _blk(gas: &Gas, raw: u64) -> Option<Gas> {
        if gas.is_empty() {
            if raw == 0 {
                None
            } else {
                Some(Gas::from_u64(raw))
            }
        } else {
            Some(*gas)
        }
    }

    #[inline]
    fn _x_value(v1: u64, v2: u32) -> u64 {
        if v1 != 0 {
            v1
        } else {
            v2 as u64
        }
    }

    pub fn dsdt(&self) -> u64 {
        Self::_x_value(self.x_dsdt, self.dsdt)
    }

    #[inline]
    pub fn gpe0_blk(&self) -> Option<Gas> {
        Self::_blk(&self.x_gpe0_blk, self.gpe0_blk as u64)
    }

    #[inline]
    pub const fn gpe0_blk_len(&self) -> usize {
        self.gpe0_blk_len as usize
    }
}
