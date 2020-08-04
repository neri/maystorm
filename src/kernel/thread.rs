// Thread
use super::scheduler::{ThreadId, TimeMeasure, Timer};

#[derive(Debug, Clone)]
pub struct Thread {
    id: ThreadId,
}

unsafe impl Sync for Thread {}

unsafe impl Send for Thread {}

impl Thread {
    pub fn id(&self) -> ThreadId {
        self.id
    }

    // pub fn name(&self) -> Option<&str> {
    //     self.name
    // }

    pub fn spawn<F>(_f: F)
    where
        F: FnOnce() -> (),
    {
        // TODO: spawn
        // f();
    }

    // pub fn current<'_>() -> Thread<'_> {
    //     Self::new()
    // }

    pub fn sleep(duration: TimeMeasure) {
        Timer::sleep(duration);
    }

    pub fn park() {}

    pub fn park_timeout(_dur: TimeMeasure) {}

    pub fn unpark(&self) {}
}
