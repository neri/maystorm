pub mod apic;
pub mod cpu;
pub mod hpet;
pub mod ps2;

pub struct Arch {}

impl Arch {
    pub(crate) fn late_init() {
        unsafe {
            let _ = ps2::Ps2::init();
        }
    }
}
