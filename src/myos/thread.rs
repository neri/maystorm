// Thread
use super::scheduler::{GlobalScheduler, ThreadId, TimeMeasure};

#[derive(Debug, Clone)]
pub struct Thread {
    id: ThreadId,
}

unsafe impl Sync for Thread {}

unsafe impl Send for Thread {}

impl Thread {
    fn new() -> Self {
        Thread { id: ThreadId(0) }
    }

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
        GlobalScheduler::sleep(duration);
    }

    pub fn usleep(us: u64) {
        Self::sleep(TimeMeasure::from_micros(us));
    }

    pub fn park() {}

    pub fn park_timeout(_dur: TimeMeasure) {}

    pub fn unpark(&self) {}
}
