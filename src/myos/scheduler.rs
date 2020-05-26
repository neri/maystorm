// Thread Scheduler

use super::arch::cpu::Cpu;
use super::arch::system::*;
use crate::myos::io::graphics::*;
use crate::myos::mem::alloc::*;
use crate::myos::mux::queue::*;
use crate::myos::mux::spinlock::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;
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

    pub(crate) unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
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

static mut GLOBAL_SCHEDULER: GlobalScheduler = GlobalScheduler::new();
const SIZE_OF_RETIRED_QUEUE: usize = 256;
const SIZE_OF_READY_QUEUE: usize = 256;

// Global System Scheduler
pub struct GlobalScheduler {
    next_thread_id: AtomicUsize,
    ready: Vec<Box<ConcurrentRingBuffer<&'static NativeThread<'static>>>>,
    retired: Option<Box<ConcurrentRingBuffer<&'static NativeThread<'static>>>>,
    lsch: Vec<Box<LocalScheduler>>,
    is_enabled: bool,
}

impl GlobalScheduler {
    const fn new() -> Self {
        Self {
            next_thread_id: AtomicUsize::new(0),
            ready: Vec::new(),
            retired: None,
            lsch: Vec::new(),
            is_enabled: false,
        }
    }

    pub(crate) unsafe fn start(system: &System, f: fn(*mut c_void) -> (), args: *mut c_void) -> ! {
        let sch = &mut GLOBAL_SCHEDULER;

        sch.retired = Some(ConcurrentRingBuffer::<&NativeThread>::with_capacity(
            SIZE_OF_RETIRED_QUEUE,
        ));
        sch.ready
            .push(ConcurrentRingBuffer::<&NativeThread>::with_capacity(
                SIZE_OF_READY_QUEUE,
            ));

        for index in 0..system.number_of_active_cpus() {
            let lsch = LocalScheduler::new(ProcessorIndex(index));
            sch.lsch.push(lsch);
        }

        for _ in 0..50 {
            Self::retire(NativeThread::new(
                Priority::Normal,
                ThreadFlags::empty(),
                Some(Self::scheduler_thread),
                null_mut(),
            ));
        }

        Self::retire(NativeThread::new(
            Priority::Normal,
            ThreadFlags::empty(),
            Some(f),
            args,
        ));

        sch.is_enabled = true;

        loop {
            Cpu::halt();
        }
    }

    fn next_thread_id() -> ThreadId {
        let sch = unsafe { &GLOBAL_SCHEDULER };
        ThreadId(sch.next_thread_id.fetch_add(1, Ordering::Relaxed))
    }

    // Perform a Preemption
    pub(crate) unsafe fn reschedule() {
        let handle = Cpu::lock_irq();
        let sch = &GLOBAL_SCHEDULER;
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

    // Get Next Thread from queue
    fn next() -> Option<&'static NativeThread<'static>> {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };
        let next = sch.ready.get_mut(0).and_then(|q| q.read());
        if next.is_none() {
            let front = sch.ready.get_mut(0).unwrap();
            let back = sch.retired.as_mut().unwrap();
            loop {
                if let Some(retired) = back.read() {
                    front.write(retired).unwrap();
                } else {
                    break None;
                }
            }
        } else {
            next
        }
    }

    // Retire Thread
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

    fn scheduler_thread(_args: *mut c_void) {
        let id = NativeThread::current_id().0 as isize;
        let mut counter: usize = 0;
        loop {
            counter += 0x040506;
            stdout()
                .fb()
                .fill_rect(Rect::new(10 * id, 5, 8, 8), Color::from(counter as u32));
        }
    }

    pub fn print_statistics() {
        let sch = unsafe { &GLOBAL_SCHEDULER };
        for lsch in &sch.lsch {
            println!("{} = {}", lsch.index.0, lsch.count.load(Ordering::Relaxed));
        }
    }
}

// Processor Local Scheduler
struct LocalScheduler {
    pub index: ProcessorIndex,
    count: AtomicUsize,
    idle: &'static NativeThread<'static>,
    current: &'static NativeThread<'static>,
    retired: Option<&'static NativeThread<'static>>,
}

impl LocalScheduler {
    fn new(index: ProcessorIndex) -> Box<Self> {
        let idle = NativeThread::new(Priority::Idle, ThreadFlags::empty(), None, null_mut());
        Box::new(Self {
            index: index,
            count: AtomicUsize::new(0),
            idle: idle,
            current: idle,
            retired: None,
        })
    }

    fn next_thread(lsch: &'static mut Self) {
        let current = lsch.current;
        let next = match GlobalScheduler::next() {
            Some(next) => next,
            None => &lsch.idle,
        };
        if current.id == next.id {
            // TODO: adjust statistics
        } else {
            lsch.count.fetch_add(1, Ordering::Relaxed);
            lsch.retired = Some(current);
            lsch.current = next;
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

static mut THREAD_POOL: ThreadPool = ThreadPool::new();

struct ThreadPool<'a> {
    vec: Vec<Box<NativeThread<'a>>>,
    lock: Spinlock,
}

impl ThreadPool<'_> {
    const fn new() -> Self {
        Self {
            vec: Vec::new(),
            lock: Spinlock::new(),
        }
    }

    fn add(thread: Box<NativeThread<'static>>) -> &mut NativeThread {
        unsafe {
            let shared = &mut THREAD_POOL;
            shared.lock.lock();
            shared.vec.push(thread);
            let len = shared.vec.len();
            let x = shared.vec.get_mut(len - 1).unwrap();
            // let x = shared.vec[len - 1];
            shared.lock.unlock();
            x
        }
    }
}

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
    ) -> &'static mut Self {
        let thread = ThreadPool::add(Box::new(Self {
            context: [0; SIZE_OF_CONTEXT],
            id: GlobalScheduler::next_thread_id(),
            priority: priority,
            attributes: flags,
            name: None,
        }));
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

    // fn current() -> &'static Self {
    //     GlobalScheduler::local_scheduler().current
    // }

    fn current_id() -> ThreadId {
        GlobalScheduler::local_scheduler().current.id
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
