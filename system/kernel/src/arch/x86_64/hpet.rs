use super::{apic::*, page::PhysicalAddress};
use crate::{mem::mmio::*, task::scheduler::*};
use core::time::Duration;

/// High Precision Event Timer
pub(super) struct Hpet {
    mmio: MmioSlice,
    main_cnt_period: u64,
    measure_div: u64,
}

impl Hpet {
    pub unsafe fn new(info: &myacpi::hpet::Hpet) -> Self {
        let mut hpet = Hpet {
            mmio: MmioSlice::from_phys(PhysicalAddress::new(info.base_address()), 0x1000).unwrap(),
            main_cnt_period: 0,
            measure_div: 0,
        };

        Irq::LPC_TIMER.register(Self::irq_handler, 0).unwrap();

        hpet.main_cnt_period = hpet.read(0) >> 32;
        hpet.write(0x10, 0);
        hpet.write(0x20, 0); // Clear all interrupts
        hpet.write(0xF0, 0); // Reset MAIN_COUNTER_VALUE
        hpet.write(0x10, 0x03); // LEG_RT_CNF | ENABLE_CNF

        hpet.measure_div = 1000_000_000 / hpet.main_cnt_period;
        hpet.write(0x100, 0x0000_004C); // Tn_INT_ENB_CNF | Tn_TYPE_CNF | Tn_VAL_SET_CNF
        hpet.write(0x108, 1000_000_000_000 / hpet.main_cnt_period);

        hpet
    }

    #[inline]
    unsafe fn read(&self, index: usize) -> u64 {
        self.mmio.read_u64(index)
    }

    #[inline]
    unsafe fn write(&self, index: usize, value: u64) {
        self.mmio.write_u64(index, value);
    }

    /// IRQ of HPET
    /// Currently, this system does not require an IRQ for HPET, but it is receiving an interrupt just in case.
    fn irq_handler(_: usize) {
        // TODO:
    }
}

impl TimerSource for Hpet {
    fn measure(&self) -> TimeSpec {
        TimeSpec((unsafe { self.read(0xF0) } / self.measure_div) as isize)
    }

    fn from_duration(&self, val: Duration) -> TimeSpec {
        TimeSpec(val.as_micros() as isize)
    }

    fn into_duration(&self, val: TimeSpec) -> Duration {
        Duration::from_micros(val.0 as u64)
    }
}
