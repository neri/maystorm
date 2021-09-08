//! Log Event Manager

use crate::system::System;
// use crate::*;
use core::fmt::Write;

pub struct Log;

impl Log {
    #[inline]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Write for Log {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        EventManager::system_log(s);
        Ok(())
    }
}

pub struct EventManager {
    //
}

impl EventManager {
    pub fn system_log(s: &str) {
        let _ = write!(System::em_console(), "{}", s);
    }
}
