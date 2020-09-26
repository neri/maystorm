// Thread Scheduler

use super::arch::cpu::Cpu;
// use crate::io::graphics::*;
use crate::mem::memory::*;
use crate::sync::spinlock::*;
use crate::system::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::num::*;
use core::ops::*;
use core::sync::atomic::*;
use crossbeam_queue::ArrayQueue;
// use bitflags::*;

extern "C" {
    fn asm_sch_switch_context(current: *mut u8, next: *mut u8);
    fn asm_sch_make_new_thread(context: *mut u8, new_sp: *mut c_void, start: usize, args: usize);
}

static mut SCHEDULER: MyScheduler = MyScheduler::new();

/// System Scheduler
pub struct MyScheduler {
    urgent: Option<Box<ThreadQueue>>,
    ready: Option<Box<ThreadQueue>>,
    retired: Option<Box<ThreadQueue>>,
    locals: Vec<Box<LocalScheduler>>,
    is_enabled: AtomicBool,
    is_frozen: AtomicBool,
}

impl MyScheduler {
    const fn new() -> Self {
        Self {
            urgent: None,
            ready: None,
            retired: None,
            locals: Vec::new(),
            is_enabled: AtomicBool::new(false),
            is_frozen: AtomicBool::new(false),
        }
    }

    pub(super) fn start(system: &System, f: fn(usize) -> (), args: usize) -> ! {
        const SIZE_OF_URGENT_QUEUE: usize = 512;
        const SIZE_OF_MAIN_QUEUE: usize = 512;

        let sch = unsafe { &mut SCHEDULER };

        sch.urgent = Some(ThreadQueue::with_capacity(SIZE_OF_URGENT_QUEUE));
        sch.ready = Some(ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE));
        sch.retired = Some(ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE));

        for index in 0..system.num_of_active_cpus() {
            sch.locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        Self::spawn_f(Self::scheduler_thread, 0, Priority::High);

        Self::spawn_f(f, args, Priority::Normal);

        sch.is_enabled.store(true, Ordering::Release);

        loop {
            unsafe {
                Cpu::halt();
            }
        }
    }

    fn next_thread_id() -> ThreadId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    // Perform a Preemption
    pub(crate) fn reschedule() {
        if Self::is_enabled() {
            unsafe {
                Cpu::without_interrupts(|| {
                    let lsch = Self::local_scheduler();
                    if lsch.current.as_ref().priority != Priority::Realtime {
                        if lsch.current.update(|current| current.quantum.consume()) {
                            LocalScheduler::next_thread(lsch);
                        }
                    }
                });
            }
        }
    }

    /// Wait for Event or Timer
    pub fn wait_for(object: Option<&SignallingObject>, duration: TimeMeasure) {
        unsafe {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler();
                if let Some(object) = object {
                    if object.load().is_none() {
                        return;
                    }
                }
                lsch.current.update(|current| {
                    current.deadline = Timer::new(duration);
                });
                LocalScheduler::next_thread(lsch);
            });
        }
    }

    pub fn signal(object: &SignallingObject) {
        if let Some(thread) = object.unbox() {
            thread.update(|thread| thread.deadline = Timer::NULL);
        }
    }

    fn local_scheduler() -> &'static mut LocalScheduler {
        let sch = unsafe { &mut SCHEDULER };
        let cpu_index = Cpu::current_processor_index().unwrap();
        sch.locals.get_mut(cpu_index.0).unwrap()
    }

    // Get Next Thread from queue
    fn next() -> Option<ThreadHandle> {
        let sch = unsafe { &mut SCHEDULER };
        if sch.is_frozen.load(Ordering::SeqCst) {
            return None;
        }
        for _ in 0..1 {
            if let Some(next) = sch.urgent.as_mut().unwrap().dequeue() {
                return Some(next);
            }
            while let Some(next) = sch.ready.as_mut().unwrap().dequeue() {
                if next.as_ref().deadline.until() {
                    MyScheduler::retire(next);
                    continue;
                } else {
                    return Some(next);
                }
            }
            let front = sch.ready.as_mut().unwrap();
            let back = sch.retired.as_mut().unwrap();
            while let Some(retired) = back.dequeue() {
                front.enqueue(retired).unwrap();
            }
        }
        None
    }

    // Retire Thread
    fn retire(thread: ThreadHandle) {
        let sch = unsafe { &mut SCHEDULER };
        let priority = thread.as_ref().priority;
        if priority != Priority::Idle {
            sch.retired.as_mut().unwrap().enqueue(thread).unwrap();
        }
    }

    fn scheduler_thread(_args: usize) {
        // TODO:
        loop {
            Self::wait_for(None, TimeMeasure::from_millis(1000));
        }
    }

    pub fn is_enabled() -> bool {
        let sch = unsafe { &SCHEDULER };
        sch.is_enabled.load(Ordering::Acquire)
    }

    pub(crate) unsafe fn freeze(force: bool) -> Result<(), ()> {
        let sch = &SCHEDULER;
        sch.is_frozen.store(true, Ordering::SeqCst);
        if force {
            // TODO:
        }
        Ok(())
    }

    pub fn spawn_f(start: ThreadStart, args: usize, priority: Priority) {
        assert!(priority.useful());
        let thread = RawThread::new(priority, Some(start), args);
        Self::retire(thread);
    }

    pub fn spawn<F>(_priority: Priority, _f: F)
    where
        F: FnOnce() -> (),
    {
        // assert!(priority.useful());
        unimplemented!();
    }
}

/// Processor Local Scheduler
struct LocalScheduler {
    #[allow(dead_code)]
    index: ProcessorIndex,
    idle: ThreadHandle,
    current: ThreadHandle,
    retired: Option<ThreadHandle>,
}

impl LocalScheduler {
    fn new(index: ProcessorIndex) -> Box<Self> {
        let idle = RawThread::new(Priority::Idle, None, 0);
        Box::new(Self {
            index,
            idle,
            current: idle,
            retired: None,
        })
    }

    fn next_thread(lsch: &'static mut Self) {
        Cpu::assert_without_interrupt();

        let current = lsch.current;
        let next = match MyScheduler::next() {
            Some(next) => next,
            None => lsch.idle,
        };
        if current.as_ref().id == next.as_ref().id {
            // TODO: adjust statistics
        } else {
            lsch.retired = Some(current);
            lsch.current = next;
            unsafe {
                asm_sch_switch_context(
                    &current.as_ref().context as *const u8 as *mut u8,
                    &next.as_ref().context as *const u8 as *mut u8,
                );
            }
            let lsch = MyScheduler::local_scheduler();
            let current = lsch.current;
            current.update(|thread| thread.deadline = Timer::NULL);
            let retired = lsch.retired.unwrap();
            lsch.retired = None;
            MyScheduler::retire(retired);
        }
    }

    fn current_thread(&self) -> ThreadHandle {
        self.current
    }
}

#[no_mangle]
pub unsafe extern "C" fn sch_setup_new_thread() {
    let lsch = MyScheduler::local_scheduler();
    if let Some(retired) = lsch.retired {
        lsch.retired = None;
        MyScheduler::retire(retired);
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ThreadId(pub usize);

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub trait TimerSource {
    fn create(&self, h: TimeMeasure) -> TimeMeasure;
    fn until(&self, h: TimeMeasure) -> bool;
    fn diff(&self, h: TimeMeasure) -> isize;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Timer {
    deadline: TimeMeasure,
}

impl Timer {
    pub const NULL: Timer = Timer {
        deadline: TimeMeasure::NULL,
    };

    pub fn new(duration: TimeMeasure) -> Self {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        Timer {
            deadline: timer.create(duration),
        }
    }

    #[must_use]
    pub fn until(self) -> bool {
        match self.deadline {
            TimeMeasure::NULL => false,
            TimeMeasure::FOREVER => true,
            _ => {
                let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
                timer.until(self.deadline)
            }
        }
    }

    pub(crate) unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    pub fn sleep(duration: TimeMeasure) {
        if MyScheduler::is_enabled() {
            MyScheduler::wait_for(None, duration);
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

    pub fn usleep(us: u64) {
        Self::sleep(TimeMeasure::from_micros(us));
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd)]
pub struct TimeMeasure(pub i64);

impl TimeMeasure {
    pub const NULL: TimeMeasure = TimeMeasure(0);
    pub const FOREVER: TimeMeasure = TimeMeasure(i64::MAX);

    pub fn is_null(self) -> bool {
        self == Self::NULL
    }

    pub fn is_forever(self) -> bool {
        self == Self::FOREVER
    }

    pub const fn from_micros(us: u64) -> Self {
        TimeMeasure(us as i64)
    }

    pub const fn from_millis(ms: u64) -> Self {
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

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Priority {
    Idle = 0,
    Low,
    Normal,
    High,
    Realtime,
}

impl Priority {
    pub fn useful(self) -> bool {
        match self {
            Priority::Idle => false,
            _ => true,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Quantum {
    current: u8,
    default: u8,
}

unsafe impl Sync for Quantum {}

impl Quantum {
    const fn new(value: u8) -> Self {
        Quantum {
            current: value,
            default: value,
        }
    }

    fn consume(&mut self) -> bool {
        if self.current > 1 {
            self.current -= 1;
            false
        } else {
            self.current = self.default;
            true
        }
    }
}

impl From<Priority> for Quantum {
    fn from(priority: Priority) -> Self {
        match priority {
            Priority::High => Quantum::new(25),
            Priority::Normal => Quantum::new(10),
            Priority::Low => Quantum::new(5),
            _ => Quantum::new(1),
        }
    }
}

static mut THREAD_POOL: ThreadPool = ThreadPool::new();

struct ThreadPool {
    vec: Vec<Box<RawThread>>,
    lock: Spinlock,
}

impl ThreadPool {
    const fn new() -> Self {
        Self {
            vec: Vec::new(),
            lock: Spinlock::new(),
        }
    }

    fn add(thread: Box<RawThread>) -> ThreadHandle {
        let shared = unsafe { &mut THREAD_POOL };
        shared.lock.lock();
        shared.vec.push(thread);
        let len = shared.vec.len();
        shared.lock.unlock();
        ThreadHandle::new(len).unwrap()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ThreadHandle(NonZeroUsize);

impl ThreadHandle {
    pub fn new(val: usize) -> Option<Self> {
        NonZeroUsize::new(val).map(|x| Self(x))
    }

    pub const fn as_usize(self) -> usize {
        self.0.get()
    }

    const fn as_index(self) -> usize {
        self.as_usize() - 1
    }

    #[inline]
    fn update<F, R>(self, f: F) -> R
    where
        F: FnOnce(&mut RawThread) -> R,
    {
        let shared = unsafe { &mut THREAD_POOL };
        let thread = shared.vec[self.as_index()].as_mut();
        f(thread)
    }

    fn as_ref(self) -> &'static RawThread {
        let shared = unsafe { &THREAD_POOL };
        shared.vec[self.as_index()].as_ref()
    }
}

const SIZE_OF_CONTEXT: usize = 512;
const SIZE_OF_STACK: usize = 0x10000;
const THREAD_NAME_LENGTH: usize = 32;

type ThreadStart = fn(usize) -> ();

#[allow(dead_code)]
struct RawThread {
    context: [u8; SIZE_OF_CONTEXT],
    id: ThreadId,
    priority: Priority,
    quantum: Quantum,
    deadline: Timer,
    name: [u8; THREAD_NAME_LENGTH],
}

#[allow(dead_code)]
impl RawThread {
    fn new(priority: Priority, start: Option<ThreadStart>, args: usize) -> ThreadHandle {
        let quantum = Quantum::from(priority);
        let handle = ThreadPool::add(Box::new(Self {
            context: [0; SIZE_OF_CONTEXT],
            id: MyScheduler::next_thread_id(),
            priority,
            quantum,
            deadline: Timer::NULL,
            name: [0; THREAD_NAME_LENGTH],
        }));
        if let Some(start) = start {
            handle.update(|thread| unsafe {
                let stack = MemoryManager::zalloc(SIZE_OF_STACK).unwrap().get() as *mut c_void;
                asm_sch_make_new_thread(
                    thread.context.as_mut_ptr(),
                    stack.add(SIZE_OF_STACK),
                    start as usize,
                    args,
                );
            });
        }
        handle
    }

    pub fn current_id() -> ThreadId {
        MyScheduler::local_scheduler().current.as_ref().id
    }

    pub fn current() -> ThreadHandle {
        MyScheduler::local_scheduler().current_thread()
    }

    pub fn exit(_exit_code: usize) -> ! {
        panic!("NO MORE THREAD!!!");
    }
}

#[derive(Debug)]
pub struct SignallingObject(AtomicUsize);

unsafe impl Sync for SignallingObject {}

unsafe impl Send for SignallingObject {}

impl SignallingObject {
    const NULL: usize = 0;

    pub fn new() -> Self {
        Self(AtomicUsize::new(RawThread::current().as_usize()))
    }

    pub fn set(&self, value: ThreadHandle) -> Result<(), ()> {
        let value = value.as_usize();
        match self
            .0
            .compare_exchange(Self::NULL, value, Ordering::SeqCst, Ordering::Relaxed)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn load(&self) -> Option<ThreadHandle> {
        ThreadHandle::new(self.0.load(Ordering::Acquire))
    }

    pub fn unbox(&self) -> Option<ThreadHandle> {
        ThreadHandle::new(self.0.swap(Self::NULL, Ordering::AcqRel))
    }

    pub fn wait(&self, duration: TimeMeasure) {
        MyScheduler::wait_for(Some(self), duration)
    }

    pub fn signal(&self) {
        MyScheduler::signal(&self)
    }
}

impl From<usize> for SignallingObject {
    fn from(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }
}

impl From<SignallingObject> for usize {
    fn from(value: SignallingObject) -> usize {
        value.0.load(Ordering::Acquire)
    }
}

struct ThreadQueue(ArrayQueue<NonZeroUsize>);

impl ThreadQueue {
    fn with_capacity(capacity: usize) -> Box<Self> {
        Box::new(Self(ArrayQueue::new(capacity)))
    }
    fn dequeue(&self) -> Option<ThreadHandle> {
        self.0.pop().ok().map(|v| ThreadHandle(v))
    }
    fn enqueue(&self, data: ThreadHandle) -> Result<(), ()> {
        self.0.push(data.0).map_err(|_| ())
    }
}
