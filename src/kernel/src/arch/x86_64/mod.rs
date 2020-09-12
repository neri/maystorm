pub mod apic;
pub mod cpu;
pub mod hpet;
pub mod ps2;

use crate::system::*;

pub struct Arch {}

impl Arch {
    pub(crate) unsafe fn init() {
        cpu::Cpu::init();

        if let acpi::InterruptModel::Apic(apic) =
            System::shared().acpi().interrupt_model.as_ref().unwrap()
        {
            apic::Apic::init(apic);
        } else {
            panic!("NO APIC");
        }
    }

    pub(crate) fn late_init() {
        unsafe {
            let _ = ps2::Ps2::init();
        }
    }
}
