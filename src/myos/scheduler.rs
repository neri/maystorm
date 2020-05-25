// Thread Scheduler

use super::arch::cpu::Cpu;
use super::arch::system::*;
use crate::myos::io::graphics::*;
use crate::myos::mem::alloc::*;
use crate::myos::mux::queue::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;
use core::cell::*;
use core::ops::*;
use core::ptr::*;
use core::sync::atomic::*;

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

extern "C" {
    fn switch_context(current: *mut u8, next: *mut u8);
    fn arch_setup_new_thread(
        context: *mut u8,
        new_sp: *mut c_void,
        start: usize,
        args: *mut c_void,
    );
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ThreadId(pub usize);

pub trait TimerSource {
    fn create(&self, h: TimeMeasure) -> TimeMeasure;
    fn until(&self, h: TimeMeasure) -> bool;
    fn diff(&self, h: TimeMeasure) -> isize;
}

#[derive(Debug, Copy, Clone)]
pub struct Timer {
    deadline: TimeMeasure,
}

impl Timer {
    pub fn new(duration: TimeMeasure) -> Self {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        Timer {
            deadline: timer.create(duration),
        }
    }

    pub fn until(&self) -> bool {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        timer.until(self.deadline)
    }
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

// Processor Local Scheduler
struct LocalScheduler {
    pub index: ProcessorIndex,
    idle: Box<NativeThread<'static>>,
    current: Option<&'static NativeThread<'static>>,
    retired: Option<&'static NativeThread<'static>>,
}

impl LocalScheduler {
    fn new(index: ProcessorIndex) -> Box<Self> {
        let idle =
            NativeThread::<'static>::new(Priority::Idle, ThreadFlags::empty(), None, null_mut());
        Box::new(Self {
            index: index,
            idle: idle,
            current: None,
            retired: None,
        })
    }

    fn next_thread(lsch: &'static mut Self) {
        let current = lsch.current.unwrap();
        let next = match GlobalScheduler::next() {
            Some(next) => next,
            None => &lsch.idle,
        };
        if current.id == next.id {
            // TODO: adjust statistics
        } else {
            lsch.retired = Some(current);
            lsch.current = Some(next);
            unsafe {
                switch_context(
                    &current.context as *const u8 as *mut u8,
                    &next.context as *const u8 as *mut u8,
                );
            }
            let lsch = GlobalScheduler::local_scheduler();
            if let Some(retired) = lsch.retired {
                lsch.retired = None;
                GlobalScheduler::retire(retired);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn dispose_new_thread() {
    let lsch = GlobalScheduler::local_scheduler();
    if let Some(retired) = lsch.retired {
        lsch.retired = None;
        GlobalScheduler::retire(retired);
    }
}

static mut GLOBAL_SCHEDULER: GlobalScheduler = GlobalScheduler::new();

// Global System Scheduler
pub(crate) struct GlobalScheduler {
    next_thread_id: AtomicUsize,
    threads: Vec<Box<NativeThread<'static>>>,
    ready: Vec<Box<ConcurrentRingBuffer<&'static NativeThread<'static>>>>,
    retired: Option<Box<ConcurrentRingBuffer<&'static NativeThread<'static>>>>,
    lsch: Vec<Box<LocalScheduler>>,
    is_enabled: bool,
}

impl GlobalScheduler {
    const fn new() -> Self {
        Self {
            next_thread_id: AtomicUsize::new(0),
            threads: Vec::new(),
            ready: Vec::new(),
            retired: None,
            lsch: Vec::new(),
            is_enabled: false,
        }
    }

    pub(crate) unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    pub(crate) unsafe fn start_threading(system: &System) {
        let sch = &mut GLOBAL_SCHEDULER;

        sch.retired = Some(ConcurrentRingBuffer::<&NativeThread>::with_capacity(256));
        let q = ConcurrentRingBuffer::<&NativeThread>::with_capacity(256);
        sch.ready.push(q);

        for index in 0..system.number_of_active_cpus() {
            let lsch = LocalScheduler::new(ProcessorIndex(index));
            sch.lsch.push(lsch);
        }
        for lsch in sch.lsch.iter_mut() {
            lsch.current = Some(&lsch.idle);
        }

        Self::add_thread(NativeThread::new(
            Priority::Normal,
            ThreadFlags::empty(),
            Some(Self::scheduler_thread),
            null_mut(),
        ));

        sch.is_enabled = true;
    }

    fn next_thread_id() -> ThreadId {
        let sch = unsafe { &GLOBAL_SCHEDULER };
        ThreadId(sch.next_thread_id.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) unsafe fn reschedule() {
        let handle = Cpu::lock_irq();
        let sch = &mut GLOBAL_SCHEDULER;
        if sch.is_enabled {
            let lsch = Self::local_scheduler();
            LocalScheduler::next_thread(lsch);
        }
        Cpu::unlock_irq(handle);
    }

    fn local_scheduler() -> &'static mut LocalScheduler {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };
        let cpu_index = System::shared().current_cpu_index().unwrap();
        sch.lsch.get_mut(cpu_index).unwrap()
    }

    fn next() -> Option<&'static NativeThread<'static>> {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };
        let next = sch.ready.get_mut(0).and_then(|q| q.read());
        if next.is_none() {
            let fb = sch.ready.get_mut(0).unwrap();
            let bb = sch.retired.as_mut().unwrap();
            loop {
                if let Some(retired) = bb.read() {
                    fb.write(retired).unwrap();
                } else {
                    break None;
                }
            }
        } else {
            next
        }
    }

    fn retire(thread: &'static NativeThread) {
        if !thread.is_idle() {
            let sch = unsafe { &mut GLOBAL_SCHEDULER };
            sch.retired.as_mut().unwrap().write(thread).unwrap();
        }
    }

    pub fn sleep(duration: TimeMeasure) {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        let deadline = timer.create(duration);
        while timer.until(deadline) {
            unsafe {
                Cpu::halt();
            }
        }
    }
    // let sch = unsafe { &mut GLOBAL_SCHEDULER };

    fn add_thread(thread: Box<NativeThread<'static>>) {
        unsafe {
            SABU_SURETTO = Some(thread);
            Self::retire(SABU_SURETTO.as_ref().unwrap());
        }
    }

    fn scheduler_thread(_args: *mut c_void) {
        let mut counter: usize = 0;
        loop {
            counter += 0x040506;
            stdout()
                .fb()
                .fill_rect(Rect::new(50, 50, 10, 10), Color::from(counter as u32));
        }
    }
}

static mut SABU_SURETTO: Option<Box<NativeThread>> = None;

bitflags! {
    struct ThreadFlags: usize {
        const RUNNING = 0b0000_0000_0000_0001;
        const ZOMBIE = 0b0000_0000_0000_0010;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Priority {
    Idle = 0,
    Low,
    Normal,
    High,
    Realtime,
}

const SIZE_OF_CONTEXT: usize = 512;
const SIZE_OF_STACK: usize = 0x10000;

type ThreadStart = fn(*mut c_void) -> ();

#[allow(dead_code)]
struct NativeThread<'a> {
    context: [u8; SIZE_OF_CONTEXT],
    id: ThreadId,
    priority: Priority,
    attributes: ThreadFlags,
    name: Option<&'a str>,
}

unsafe impl Sync for NativeThread<'_> {}

impl NativeThread<'_> {
    fn new(
        priority: Priority,
        flags: ThreadFlags,
        start: Option<ThreadStart>,
        args: *mut c_void,
    ) -> Box<Self> {
        let thread = Box::new(Self {
            context: [0; SIZE_OF_CONTEXT],
            id: GlobalScheduler::next_thread_id(),
            priority: priority,
            attributes: flags,
            name: None,
        });
        if let Some(start) = start {
            unsafe {
                let stack = CustomAlloc::zalloc(SIZE_OF_STACK).unwrap().as_ptr();
                arch_setup_new_thread(
                    &thread.context as *const u8 as *mut u8,
                    stack.add(SIZE_OF_STACK),
                    start as usize,
                    args,
                );
            }
        }
        thread
    }

    #[inline]
    fn is_idle(&self) -> bool {
        self.priority == Priority::Idle
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Irql {
    Passive = 0,
    Dispatch,
    Device,
    High,
}

impl Irql {
    pub fn raise(new_irql: Irql) -> Result<Irql, ()> {
        let old_irql = Self::current();
        if old_irql > new_irql {
            panic!("IRQL_NOT_LESS_OR_EQUAL");
        }
        Ok(old_irql)
    }

    pub fn lower(_new_irql: Irql) -> Result<(), ()> {
        Ok(())
    }

    pub fn current() -> Irql {
        Irql::Passive
    }
}
