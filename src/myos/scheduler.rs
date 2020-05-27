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
    fn sch_switch_context(current: *mut u8, next: *mut u8);
    fn sch_make_new_thread(context: *mut u8, new_sp: *mut c_void, start: usize, args: *mut c_void);
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ThreadId(pub usize);

unsafe impl Sync for ThreadId {}

unsafe impl Send for ThreadId {}

impl ThreadId {
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

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
    pub const NULL: Timer = Timer {
        deadline: TimeMeasure(0),
    };

    pub fn new(duration: TimeMeasure) -> Self {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        Timer {
            deadline: timer.create(duration),
        }
    }

    pub fn until(&self) -> bool {
        if self.is_null() {
            false
        } else {
            let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
            timer.until(self.deadline)
        }
    }

    pub(crate) unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    pub fn is_null(&self) -> bool {
        self.deadline == Self::NULL.deadline
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

    pub const fn as_micros(self) -> i64 {
        self.0 as i64
    }

    pub const fn as_millis(self) -> i64 {
        self.0 as i64 / 1000
    }

    pub const fn as_secs(self) -> i64 {
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

/// System Global Scheduler
pub struct GlobalScheduler {
    next_thread_id: AtomicUsize,
    ready: Vec<Box<ConcurrentRingBuffer<&'static NativeThread>>>,
    retired: Option<Box<ConcurrentRingBuffer<&'static NativeThread>>>,
    locals: Vec<Box<LocalScheduler>>,
    is_enabled: bool,
}

impl GlobalScheduler {
    const fn new() -> Self {
        Self {
            next_thread_id: AtomicUsize::new(0),
            ready: Vec::new(),
            retired: None,
            locals: Vec::new(),
            is_enabled: false,
        }
    }

    pub(crate) fn start(system: &System, f: fn(*mut c_void) -> (), args: *mut c_void) {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };

        sch.retired = Some(ConcurrentRingBuffer::<&NativeThread>::with_capacity(
            SIZE_OF_RETIRED_QUEUE,
        ));
        sch.ready
            .push(ConcurrentRingBuffer::<&NativeThread>::with_capacity(
                SIZE_OF_READY_QUEUE,
            ));

        for index in 0..system.number_of_active_cpus() {
            sch.locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        for _ in 0..10 {
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

        // loop {
        //     GlobalScheduler::wait_for(None, TimeMeasure(0));
        // }
        loop {
            unsafe {
                Cpu::halt();
            }
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
            if lsch.current.priority != Priority::Realtime {
                // FIXME: wicked code
                let current_mut = &mut *(lsch.current as *const NativeThread as *mut NativeThread);
                if current_mut.quantum.consume() {
                    LocalScheduler::next_thread(lsch);
                }
            }
        }
        Cpu::unlock_irq(handle);
    }

    pub fn wait_for(_: Option<NonNull<usize>>, duration: TimeMeasure) {
        unsafe {
            let handle = Cpu::lock_irq();
            let lsch = Self::local_scheduler();
            {
                // FIXME: wicked code
                let current_mut = &mut *(lsch.current as *const NativeThread as *mut NativeThread);
                current_mut.deadline = Timer::new(duration);
            }
            LocalScheduler::next_thread(lsch);
            Cpu::unlock_irq(handle);
        }
    }

    fn local_scheduler() -> &'static mut LocalScheduler {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };
        let cpu_index = System::shared().current_cpu_index().unwrap();
        sch.locals.get_mut(cpu_index).unwrap()
    }

    // Get Next Thread from queue
    fn next() -> Option<&'static NativeThread> {
        let sch = unsafe { &mut GLOBAL_SCHEDULER };
        while let Some(next) = sch.ready.get_mut(0).and_then(|q| q.read()) {
            if next.deadline.until() {
                GlobalScheduler::retire(next);
                continue;
            } else {
                return Some(next);
            }
        }
        let front = sch.ready.get_mut(0).unwrap();
        let back = sch.retired.as_mut().unwrap();
        loop {
            if let Some(retired) = back.read() {
                front.write(retired).unwrap();
            } else {
                return None;
            }
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
        let sch = unsafe { &GLOBAL_SCHEDULER };
        if sch.is_enabled {
            Self::wait_for(None, duration);
        } else {
            let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
            let deadline = timer.create(duration);
            while timer.until(deadline) {
                unsafe {
                    Cpu::halt();
                }
            }
        }
    }

    fn scheduler_thread(_args: *mut c_void) {
        // TODO:
        let id = NativeThread::current_id().0 as isize;
        let mut counter: usize = 0;
        loop {
            counter += 0x040506;
            stdout()
                .fb()
                .fill_rect(Rect::new(10 * id, 5, 8, 8), Color::from(counter as u32));
            Self::wait_for(None, TimeMeasure::from_mills(5));
        }
    }

    pub fn print_statistics() {
        let sch = unsafe { &GLOBAL_SCHEDULER };
        for lsch in &sch.locals {
            println!("{} = {}", lsch.index.0, lsch.count.load(Ordering::Relaxed));
        }
    }
}

// Processor Local Scheduler
struct LocalScheduler {
    pub index: ProcessorIndex,
    count: AtomicUsize,
    idle: &'static NativeThread,
    current: &'static NativeThread,
    retired: Option<&'static NativeThread>,
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
        unsafe {
            llvm_asm!(".byte 0xb8, 0xef, 0xbe, 0xad, 0xde":::"eax");
        }
        Cpu::check_irq().ok().unwrap();
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
                // FIXME: wicked code
                let current_mut = &mut *(current as *const NativeThread as *mut NativeThread);
                current_mut.quantum = current_mut.default_quantum;
                sch_switch_context(
                    &current.context as *const u8 as *mut u8,
                    &next.context as *const u8 as *mut u8,
                );
            }
            let lsch = GlobalScheduler::local_scheduler();
            unsafe {
                let current_mut = &mut *(current as *const NativeThread as *mut NativeThread);
                current_mut.deadline = Timer::NULL;
            }
            let retired = lsch.retired.unwrap();
            // if let Some(retired) = lsch.retired {
            lsch.retired = None;
            GlobalScheduler::retire(retired);
            // }
        }
    }
}

#[no_mangle]
pub extern "C" fn sch_setup_new_thread() {
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

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Priority {
    Idle = 0,
    Low,
    Normal,
    High,
    Realtime,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Quantum(pub usize);

unsafe impl Sync for Quantum {}

impl Quantum {
    fn consume(&mut self) -> bool {
        if self.0 > 1 {
            self.0 -= 1;
            false
        } else {
            true
        }
    }
}

impl From<Priority> for Quantum {
    fn from(priority: Priority) -> Self {
        match priority {
            Priority::High => Quantum(25),
            Priority::Normal => Quantum(10),
            Priority::Low => Quantum(5),
            _ => Quantum(1),
        }
    }
}

static mut THREAD_POOL: ThreadPool = ThreadPool::new();

struct ThreadPool {
    vec: Vec<Box<NativeThread>>,
    lock: Spinlock,
}

impl ThreadPool {
    const fn new() -> Self {
        Self {
            vec: Vec::new(),
            lock: Spinlock::new(),
        }
    }

    fn add(thread: Box<NativeThread>) -> &'static mut NativeThread {
        unsafe {
            let shared = &mut THREAD_POOL;
            shared.lock.lock();
            shared.vec.push(thread);
            let len = shared.vec.len();
            let x = shared.vec.get_mut(len - 1).unwrap();
            shared.lock.unlock();
            x
        }
    }
}

const SIZE_OF_CONTEXT: usize = 512;
const SIZE_OF_STACK: usize = 0x10000;

type ThreadStart = fn(*mut c_void) -> ();

#[allow(dead_code)]
pub(crate) struct NativeThread {
    context: [u8; SIZE_OF_CONTEXT],
    id: ThreadId,
    priority: Priority,
    quantum: Quantum,
    default_quantum: Quantum,
    deadline: Timer,
    attributes: ThreadFlags,
}

static mut TEMP_STACK_POINTER: AtomicUsize = AtomicUsize::new(0);

fn temp_stack() -> *mut core::ffi::c_void {
    unsafe {
        let x = TEMP_STACK_POINTER.fetch_add(0x8000, Ordering::Relaxed) as usize;
        x as *const c_void as *mut c_void
    }
}

unsafe impl Sync for NativeThread {}

impl NativeThread {
    fn new(
        priority: Priority,
        flags: ThreadFlags,
        start: Option<ThreadStart>,
        args: *mut c_void,
    ) -> &'static mut Self {
        let quantum = Quantum::from(priority);
        let thread = ThreadPool::add(Box::new(Self {
            context: [0; SIZE_OF_CONTEXT],
            id: GlobalScheduler::next_thread_id(),
            priority: priority,
            quantum: quantum,
            default_quantum: quantum,
            deadline: Timer::NULL,
            attributes: flags,
        }));
        if let Some(start) = start {
            unsafe {
                // let stack = CustomAlloc::zalloc(SIZE_OF_STACK).unwrap().as_ptr();
                let stack = temp_stack();
                sch_make_new_thread(
                    thread.context.as_mut_ptr(),
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

    pub fn current_id() -> ThreadId {
        GlobalScheduler::local_scheduler().current.id
    }

    pub fn exit(_exit_code: usize) -> ! {
        panic!("NO MORE THREAD!!!");
    }

    #[inline]
    fn is_idle(&self) -> bool {
        self.priority == Priority::Idle
    }
}

#[non_exhaustive]
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
