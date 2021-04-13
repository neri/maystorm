// Real Time Clock

use super::cpu::*;
use crate::sync::spinlock::*;
use crate::task::scheduler::*;
use megstd::time::SystemTime;

static mut RTC: Rtc = Rtc::new();

pub(super) struct Rtc {
    base: u64,
    offset: u64,
    lock: Spinlock,
}

impl Rtc {
    const fn new() -> Self {
        Self {
            base: 0,
            offset: 0,
            lock: Spinlock::new(),
        }
    }

    pub unsafe fn init() {
        RTC.base = Self::fetch_time();
        RTC.offset = Timer::monotonic().as_nanos() as u64;
    }

    pub fn system_time() -> SystemTime {
        let shared = unsafe { &RTC };

        let nanos_per_sec = 1_000_000_000;
        let diff = Timer::monotonic().as_nanos() as u64 - shared.offset;
        let secs = shared.base + diff / nanos_per_sec;
        let nanos = (diff % nanos_per_sec) as u32;

        SystemTime { secs, nanos }
    }

    unsafe fn fetch_time() -> u64 {
        RTC.lock.synchronized(|| {
            Cpu::without_interrupts(|| loop {
                let time1 = Self::read_time();
                let time2 = Self::read_time();
                if time1 == time2 {
                    break time1;
                }
            })
        })
    }

    unsafe fn read_time() -> u64 {
        let sec = Cmos::Seconds.read_bcd();
        let min = Cmos::Minutes.read_bcd();
        let hour = Cmos::Hours.read_bcd();
        sec + min * 60 + hour * 3600
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
enum Cmos {
    Seconds = 0,
    SecondsAlarm,
    Minutes,
    MinutesAlarm,
    Hours,
    HoursAlarm,
    DayOfWeek,
    DayOfMonth,
    Month,
    Year,
}

#[allow(dead_code)]
impl Cmos {
    unsafe fn read_bcd(&self) -> u64 {
        let bcd = self.read() as u64;
        (bcd & 0x0F) + (bcd / 16) * 10
    }

    unsafe fn read(&self) -> u8 {
        let mut result: u8;
        asm!("
            out 0x70, al
            in al, 0x71
            ", inlateout("al") *self as u8 => result);
        result
    }

    unsafe fn write(&self, data: u8) {
        asm!("
            mov al, {0}
            out 0x70, al
            mov al, {1}
            out 0x71, al
            ", in(reg_byte) *self as u8, in(reg_byte) data, out("al") _);
    }
}
