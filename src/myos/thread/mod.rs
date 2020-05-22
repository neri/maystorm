// Thread

use super::arch::cpu::Cpu;
use alloc::boxed::Box;
use alloc::vec::*;
use core::ffi::c_void;
use core::ops::*;
use core::ptr::*;

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub struct Thread {
    id: usize,
}

unsafe impl Sync for Thread {}

impl Thread {
    pub fn new() -> Self {
        Thread { id: 0 }
    }

    pub fn sleep(duration: TimeMeasure) {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        let deadline = timer.create(duration);
        while timer.until(deadline) {
            Cpu::relax();
        }
    }

    pub fn usleep(us: u64) {
        Self::sleep(TimeMeasure::from_micros(us));
    }

    pub unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }
}

pub trait TimerSource {
    fn create(&self, h: TimeMeasure) -> TimeMeasure;
    fn until(&self, h: TimeMeasure) -> bool;
    fn diff(&self, h: TimeMeasure) -> isize;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct TimeMeasure(pub i64);

impl TimeMeasure {
    pub const fn from_micros(us: u64) -> Self {
        TimeMeasure(us as i64)
    }

    pub const fn from_mills(ms: u64) -> Self {
        TimeMeasure(ms as i64 * 1000)
    }

    pub const fn from_secs(s: u64) -> Self {
        TimeMeasure(s as i64 * 1000_000)
    }

    pub const fn as_micros(&self) -> i64 {
        self.0 as i64
    }

    pub const fn as_millis(&self) -> i64 {
        self.0 as i64 / 1000
    }

    pub const fn as_secs(&self) -> i64 {
        self.0 as i64 / 1000_000
    }
}

impl Add<isize> for TimeMeasure {
    type Output = Self;
    fn add(self, rhs: isize) -> Self {
        Self(self.0 + rhs as i64)
    }
}

impl Sub<isize> for TimeMeasure {
    type Output = Self;
    fn sub(self, rhs: isize) -> Self {
        Self(self.0 - rhs as i64)
    }
}
