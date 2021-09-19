//! Log Event Manager

use crate::{sync::fifo::AsyncEventQueue, system::System};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use core::{fmt::Write, mem::MaybeUninit, pin::Pin};
use futures_util::Future;

#[macro_export]
macro_rules! notify {
    ($($arg:tt)*) => {
        let mut sb = megstd::string::Sb255::new();
        write!(sb, $($arg)*).unwrap();
        log::EventManager::notify_simple_message(sb.as_str());
    };
}

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

static mut EVENT_MANAGER: MaybeUninit<EventManager> = MaybeUninit::uninit();

pub struct EventManager {
    message_queue: AsyncEventQueue<String>,
}

impl EventManager {
    fn new() -> Self {
        Self {
            message_queue: AsyncEventQueue::new(1000),
        }
    }

    pub(crate) fn init() {
        unsafe {
            EVENT_MANAGER.write(Self::new());
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*EVENT_MANAGER.as_ptr() }
    }

    pub fn system_log(s: &str) {
        let _ = write!(System::em_console(), "{}", s);
    }

    pub fn notify_simple_message(payload: &str) {
        let shared = Self::shared();
        shared.message_queue.post(payload.to_string()).unwrap();
    }

    pub fn monitor_notification() -> Pin<Box<dyn Future<Output = Option<String>>>> {
        let shared = Self::shared();
        Box::pin(shared.message_queue.wait_event())
    }
}
