// Thread Scheduler

use super::executor::Executor;
use super::*;
use crate::arch::cpu::Cpu;
use crate::mem::memory::*;
use crate::mem::string::*;
use crate::rt::*;
use crate::sync::semaphore::*;
use crate::sync::spinlock::*;
use crate::system::*;
use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::*;
use core::fmt::Write;
use core::num::*;
use core::ops::*;
use core::sync::atomic::*;
use core::time::Duration;
use crossbeam_queue::ArrayQueue;

const THRESHOLD_SAVING: usize = 900;
const THRESHOLD_FULL_THROTTLE_MODE: usize = 450;

extern "C" {
    fn asm_sch_switch_context(current: *mut u8, next: *mut u8);
    fn asm_sch_make_new_thread(context: *mut u8, new_sp: *mut c_void, start: usize, args: usize);
}

static mut SCHEDULER: Option<Box<MyScheduler>> = None;

static SCHEDULER_ENABLED: AtomicBool = AtomicBool::new(false);

/// System Scheduler
pub struct MyScheduler {
    urgent: ThreadQueue,
    ready: ThreadQueue,

    locals: Vec<Box<LocalScheduler>>,

    pool: ThreadPool,

    usage: AtomicUsize,
    usage_total: AtomicUsize,
    is_frozen: AtomicBool,
    state: SchedulerState,

    next_timer: Timer,
    sem_timer: Semaphore,
    timer_queue: ArrayQueue<TimerEvent>,
}

#[allow(non_camel_case_types)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SchedulerState {
    Disabled = 0,
    Saving,
    Running,
    FullThrottle,
    MAX,
}

impl MyScheduler {
    /// Start thread scheduler and sleep forver
    pub(crate) fn start(f: fn(usize) -> (), args: usize) -> ! {
        const SIZE_OF_URGENT_QUEUE: usize = 512;
        const SIZE_OF_MAIN_QUEUE: usize = 512;

        let urgent = ThreadQueue::with_capacity(SIZE_OF_URGENT_QUEUE);
        let ready = ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE);
        let pool = ThreadPool::default();

        let locals = Vec::new();

        unsafe {
            SCHEDULER = Some(Box::new(Self {
                urgent,
                ready,
                locals,
                pool,
                usage: AtomicUsize::new(0),
                usage_total: AtomicUsize::new(0),
                is_frozen: AtomicBool::new(false),
                state: SchedulerState::Running,
                next_timer: Timer::JUST,
                sem_timer: Semaphore::new(0),
                timer_queue: ArrayQueue::new(100),
            }));
        }

        let sch = Self::shared();
        for index in 0..System::num_of_active_cpus() {
            sch.locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        SpawnOption::with_priority(Priority::Realtime).spawn(
            Self::scheduler_thread,
            0,
            "Scheduler",
        );

        SpawnOption::with_priority(Priority::Normal).spawn(f, args, "kernel task");

        SCHEDULER_ENABLED.store(true, Ordering::SeqCst);

        loop {
            unsafe {
                Cpu::halt();
            }
        }
    }

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a mut Self {
        unsafe { SCHEDULER.as_mut().unwrap() }
    }

    /// Get the current process if possible
    #[inline]
    pub fn current_pid() -> Option<ProcessId> {
        if Self::is_enabled() {
            Self::current_thread().map(|thread| thread.as_ref().pid)
        } else {
            None
        }
    }

    /// Get the current thread running on the current processor
    #[inline]
    pub fn current_thread() -> Option<ThreadHandle> {
        Self::local_scheduler().map(|sch| sch.current_thread())
    }

    /// Get the personality instance associated with the current thread
    #[inline]
    pub fn current_personality<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&mut Box<dyn Personality>) -> R,
    {
        Self::current_thread()
            .and_then(|thread| thread.update(|thread| thread.personality.as_mut().map(|v| f(v))))
    }

    /// Perform the preemption
    pub(crate) unsafe fn reschedule() {
        if Self::is_enabled() {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler().unwrap();
                if lsch.current.as_ref().priority != Priority::Realtime {
                    if lsch.current.update(|current| current.quantum.consume()) {
                        LocalScheduler::switch_context(lsch);
                    }
                }
            });
        }
    }

    pub fn wait_for(object: Option<&SignallingObject>, duration: Duration) {
        unsafe {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler().unwrap();
                let current = lsch.current;
                if let Some(object) = object {
                    let _ = object.set(current);
                }
                if duration.as_nanos() > 0 {
                    Timer::sleep(duration);
                } else {
                    MyScheduler::sleep();
                }
            });
        }
    }

    pub fn sleep() {
        unsafe {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler().unwrap();
                let current = lsch.current;
                current.as_ref().attribute.insert(ThreadAttributes::ASLEEP);
                LocalScheduler::switch_context(lsch);
            });
        }
    }

    pub fn yield_thread() {
        unsafe {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler().unwrap();
                LocalScheduler::switch_context(lsch);
            });
        }
    }

    /// Get the scheduler for the current processor
    #[inline]
    fn local_scheduler() -> Option<&'static mut Box<LocalScheduler>> {
        match unsafe { SCHEDULER.as_mut() } {
            Some(sch) => {
                Cpu::current_processor_index().and_then(move |index| sch.locals.get_mut(index.0))
            }
            None => None,
        }
    }

    // Get Next Thread from queue
    fn next(index: ProcessorIndex) -> Option<ThreadHandle> {
        let sch = Self::shared();
        if sch.is_frozen.load(Ordering::SeqCst) {
            return None;
        }
        if System::cpu(index.0).processor_type() != ProcessorCoreType::Physical
            && sch.state < SchedulerState::FullThrottle
        {
            return None;
        }
        if !sch.next_timer.until() {
            sch.sem_timer.signal();
        }
        if let Some(next) = sch.urgent.dequeue() {
            return Some(next);
        }
        if let Some(next) = sch.ready.dequeue() {
            return Some(next);
        }
        None
    }

    // Retire Thread
    fn retire(thread: ThreadHandle) {
        let handle = thread;
        let sch = Self::shared();
        let thread = handle.as_ref();
        if thread.priority == Priority::Idle {
            return;
        } else if thread.attribute.contains(ThreadAttributes::ZOMBIE) {
            ThreadPool::drop_thread(handle);
        } else if thread.attribute.test_and_clear(ThreadAttributes::AWAKE) {
            thread.attribute.remove(ThreadAttributes::ASLEEP);
            sch.ready.enqueue(handle).unwrap();
        } else if thread.attribute.contains(ThreadAttributes::ASLEEP) {
            thread.attribute.remove(ThreadAttributes::QUEUED);
        } else {
            sch.ready.enqueue(handle).unwrap();
        }
    }

    // Add thread to the queue
    fn add(thread: ThreadHandle) {
        let handle = thread;
        let sch = Self::shared();
        let thread = handle.as_ref();
        if thread.priority == Priority::Idle || thread.attribute.contains(ThreadAttributes::ZOMBIE)
        {
            return;
        }
        if !thread.attribute.test_and_set(ThreadAttributes::QUEUED) {
            if thread.attribute.test_and_clear(ThreadAttributes::AWAKE) {
                thread.attribute.remove(ThreadAttributes::ASLEEP);
            }
            sch.ready.enqueue(handle).unwrap();
        }
    }

    /// Schedule a timer event
    pub fn schedule_timer(event: TimerEvent) -> Result<(), TimerEvent> {
        let shared = Self::shared();
        shared
            .timer_queue
            .push(event)
            .map(|_| shared.sem_timer.signal())
            .map_err(|e| e.0)
    }

    /// Scheduler
    fn scheduler_thread(_args: usize) {
        let shared = Self::shared();

        SpawnOption::new().spawn_f(Self::statistics_thread, 0, "stat");

        let mut events: Vec<TimerEvent> = Vec::with_capacity(100);
        loop {
            if let Some(event) = shared.timer_queue.pop().ok() {
                events.push(event);
                while let Some(event) = shared.timer_queue.pop().ok() {
                    events.push(event);
                }
                events.sort_by(|a, b| a.timer.deadline.cmp(&b.timer.deadline));
            }

            while let Some(event) = events.first() {
                if event.until() {
                    break;
                } else {
                    events.remove(0).fire();
                }
            }

            if let Some(event) = events.first() {
                shared.next_timer = event.timer;
            }
            shared.sem_timer.wait();
        }
    }

    /// Measuring Statistics
    fn statistics_thread(_: usize) {
        let shared = Self::shared();

        let expect = 1_000_000;
        let interval = Duration::from_micros(expect as u64);
        let mut measure = Timer::measure();
        loop {
            Timer::sleep(interval);

            let now = Timer::measure();
            let actual = now - measure;
            let actual1000 = actual as usize * 1000;

            let mut usage = 0;
            for thread in shared.pool.data.values() {
                let load0 = thread.load0.swap(0, Ordering::SeqCst);
                let load = usize::min(load0 as usize * expect as usize / actual1000, 1000);
                thread.load.store(load as u32, Ordering::SeqCst);
                if thread.priority != Priority::Idle {
                    usage += load;
                }
            }

            let num_cpu = System::num_of_active_cpus();
            let usage_total = usize::min(usage, num_cpu * 1000);
            let usage_per_cpu = usize::min(usage / num_cpu, 1000);
            shared.usage_total.store(usage_total, Ordering::SeqCst);
            shared.usage.store(usage_per_cpu, Ordering::SeqCst);

            let new_state: SchedulerState;
            if usage_per_cpu > THRESHOLD_FULL_THROTTLE_MODE {
                new_state = SchedulerState::FullThrottle;
            } else if usage_total > THRESHOLD_SAVING {
                new_state = SchedulerState::Running;
            } else {
                new_state = SchedulerState::Saving;
            }
            if new_state != shared.state {
                shared.state = new_state;
            }

            measure = now;
        }
    }

    pub fn usage_per_cpu() -> usize {
        let sch = Self::shared();
        sch.usage.load(Ordering::Relaxed)
    }

    pub fn usage_total() -> usize {
        let sch = Self::shared();
        sch.usage_total.load(Ordering::Relaxed)
    }

    /// Returns the current state of the scheduler.
    pub fn current_state() -> SchedulerState {
        if Self::is_enabled() {
            Self::shared().state
        } else {
            SchedulerState::Disabled
        }
    }

    /// Returns whether or not the thread scheduler is working.
    fn is_enabled() -> bool {
        unsafe { &SCHEDULER }.is_some() && SCHEDULER_ENABLED.load(Ordering::SeqCst)
    }

    /// All threads will stop.
    pub(crate) unsafe fn freeze(force: bool) -> Result<(), ()> {
        let sch = Self::shared();
        sch.is_frozen.store(true, Ordering::SeqCst);
        if force {
            // TODO:
        }
        Ok(())
    }

    fn spawn_f(
        start: ThreadStart,
        args: usize,
        name: &str,
        options: SpawnOption,
    ) -> Option<ThreadHandle> {
        assert!(options.priority.is_useful());
        let pid = if options.raise_pid {
            RuntimeEnvironment::raise_pid()
        } else {
            Self::current_pid().unwrap_or(ProcessId(0))
        };
        let thread = RawThread::new(
            pid,
            options.priority,
            name,
            Some(start),
            args,
            options.personality,
        );
        Self::add(thread);
        Some(thread)
    }

    /// Spawning asynchronous tasks
    pub fn spawn_async(task: Task) {
        Self::current_thread().unwrap().update(|thread| {
            if thread.executor.is_none() {
                thread.executor = Some(Executor::new());
            }
            thread.executor.as_mut().unwrap().spawn(task);
        });
    }

    /// Performing Asynchronous Tasks
    pub fn perform_tasks() -> ! {
        Self::current_thread().unwrap().update(|thread| {
            thread.executor.as_mut().map(|v| v.run());
        });
        Self::exit();
    }

    pub fn exit() -> ! {
        Self::current_thread().unwrap().update(|t| t.exit());
        unreachable!()
    }

    pub fn get_idle_statistics(vec: &mut Vec<u32>) {
        let sch = Self::shared();
        vec.clear();
        for thread in sch.pool.data.values() {
            if thread.priority != Priority::Idle {
                break;
            }
            vec.push(thread.load.load(Ordering::Relaxed));
        }
    }

    pub fn print_statistics(sb: &mut StringBuffer, exclude_idle: bool) {
        let sch = Self::shared();
        sb.clear();
        writeln!(sb, "PID PRI %CPU TIME     NAME").unwrap();
        for thread in sch.pool.data.values() {
            if exclude_idle && thread.priority == Priority::Idle {
                continue;
            }

            let load = u32::min(thread.load.load(Ordering::Relaxed), 999);
            let load0 = load % 10;
            let load1 = load / 10;

            let time = thread.cpu_time.load(Ordering::Relaxed) / 10_000;
            let dsec = time % 100;
            let sec = time / 100 % 60;
            let min = time / 60_00 % 60;
            let hour = time / 3600_00;

            write!(
                sb,
                "{:3} {:3} {:2}.{:1}",
                thread.pid.0, thread.priority as usize, load1, load0,
            )
            .unwrap();

            if hour > 0 {
                write!(sb, " {:02}:{:02}:{:02}", hour, min, sec,).unwrap();
            } else {
                write!(sb, " {:02}:{:02}.{:02}", min, sec, dsec,).unwrap();
            }

            write!(sb, " {}", thread.attribute).unwrap();

            match thread.name() {
                Some(name) => writeln!(sb, " {}", name,).unwrap(),
                None => writeln!(sb, " ({})", thread.handle.as_usize(),).unwrap(),
            }
        }
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
        let mut sb = Sb255::new();
        sformat!(sb, "(Idle Core #{})", index.0);
        let idle = RawThread::new(ProcessId(0), Priority::Idle, sb.as_str(), None, 0, None);
        Box::new(Self {
            index,
            idle,
            current: idle,
            retired: None,
        })
    }

    unsafe fn switch_context(lsch: &'static mut Self) {
        Cpu::assert_without_interrupt();

        let current = lsch.current;
        let next = MyScheduler::next(lsch.index).unwrap_or(lsch.idle);
        current.update(|thread| {
            let now = Timer::measure();
            let diff = now - thread.measure.load(Ordering::SeqCst);
            thread.cpu_time.fetch_add(diff, Ordering::SeqCst);
            thread.load0.fetch_add(diff as u32, Ordering::SeqCst);
            thread.measure.store(now, Ordering::SeqCst);
        });
        if current.as_ref().handle != next.as_ref().handle {
            lsch.retired = Some(current);
            lsch.current = next;

            //-//-//-//-//
            asm_sch_switch_context(
                &current.as_ref().context as *const _ as *mut _,
                &next.as_ref().context as *const _ as *mut _,
            );
            //-//-//-//-//

            let lsch = MyScheduler::local_scheduler().unwrap();
            let current = lsch.current;
            current.update(|thread| {
                thread.attribute.remove(ThreadAttributes::AWAKE);
                thread.attribute.remove(ThreadAttributes::ASLEEP);
                thread.measure.store(Timer::measure(), Ordering::SeqCst);
                thread.deadline = Timer::JUST;
                // thread.quantum.reset();
            });
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
    let lsch = MyScheduler::local_scheduler().unwrap();
    let current = lsch.current;
    current.update(|thread| {
        thread.measure.store(Timer::measure(), Ordering::SeqCst);
    });
    if let Some(retired) = lsch.retired {
        lsch.retired = None;
        MyScheduler::retire(retired);
    }
}

pub struct SpawnOption {
    pub priority: Priority,
    pub raise_pid: bool,
    pub personality: Option<Box<dyn Personality>>,
}

impl SpawnOption {
    #[inline]
    pub const fn new() -> Self {
        Self {
            priority: Priority::Normal,
            raise_pid: false,
            personality: None,
        }
    }

    #[inline]
    pub const fn with_priority(priority: Priority) -> Self {
        Self {
            priority,
            raise_pid: false,
            personality: None,
        }
    }

    #[inline]
    pub fn personality(mut self, personality: Box<dyn Personality>) -> Self {
        self.personality = Some(personality);
        self
    }

    #[inline]
    pub fn spawn_f(self, start: fn(usize), args: usize, name: &str) -> Option<ThreadHandle> {
        MyScheduler::spawn_f(start, args, name, self)
    }

    #[inline]
    pub fn spawn(mut self, start: fn(usize), args: usize, name: &str) -> Option<ThreadHandle> {
        self.raise_pid = true;
        MyScheduler::spawn_f(start, args, name, self)
    }
}

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub type TimeSpec = u64;

pub trait TimerSource {
    /// Create timer object from duration
    fn create(&self, duration: Duration) -> TimeSpec;

    /// Is that a timer before the deadline?
    fn until(&self, deadline: TimeSpec) -> bool;

    /// Get the value of the monotonic timer in microseconds
    fn monotonic(&self) -> Duration;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Timer {
    deadline: TimeSpec,
}

impl Timer {
    pub const JUST: Timer = Timer { deadline: 0 };

    #[inline]
    pub fn new(duration: Duration) -> Self {
        let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
        Timer {
            deadline: timer.create(duration),
        }
    }

    #[inline]
    pub const fn is_just(&self) -> bool {
        self.deadline == 0
    }

    #[inline]
    pub fn until(&self) -> bool {
        if self.is_just() {
            false
        } else {
            let timer = unsafe { TIMER_SOURCE.as_ref().unwrap() };
            timer.until(self.deadline)
        }
    }

    #[inline]
    pub(crate) unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    #[track_caller]
    pub fn sleep(duration: Duration) {
        if MyScheduler::is_enabled() {
            let timer = Timer::new(duration);
            let mut event = TimerEvent::one_shot(timer);
            while timer.until() {
                match MyScheduler::schedule_timer(event) {
                    Ok(()) => {
                        MyScheduler::sleep();
                        return;
                    }
                    Err(e) => {
                        event = e;
                        MyScheduler::yield_thread();
                    }
                }
            }
        } else {
            panic!("Scheduler unavailable");
        }
    }

    #[inline]
    pub fn usleep(us: u64) {
        Self::sleep(Duration::from_micros(us));
    }

    #[inline]
    pub fn msleep(ms: u64) {
        Self::sleep(Duration::from_millis(ms));
    }

    #[inline]
    pub fn monotonic() -> Duration {
        unsafe { TIMER_SOURCE.as_ref() }.unwrap().monotonic()
    }

    #[inline]
    pub fn measure() -> u64 {
        Self::monotonic().as_micros() as u64
    }
}

pub struct TimerEvent {
    timer: Timer,
    timer_type: TimerType,
}

#[derive(Debug, Copy, Clone)]
pub enum TimerType {
    OneShot(ThreadHandle),
    Window(WindowHandle, usize),
}

#[allow(dead_code)]
impl TimerEvent {
    pub fn one_shot(timer: Timer) -> Self {
        Self {
            timer,
            timer_type: TimerType::OneShot(MyScheduler::current_thread().unwrap()),
        }
    }

    pub fn window(window: WindowHandle, timer_id: usize, timer: Timer) -> Self {
        Self {
            timer,
            timer_type: TimerType::Window(window, timer_id),
        }
    }

    pub fn until(&self) -> bool {
        self.timer.until()
    }

    pub fn fire(self) {
        match self.timer_type {
            TimerType::OneShot(thread) => thread.wake(),
            TimerType::Window(window, timer_id) => {
                window.post(WindowMessage::Timer(timer_id)).unwrap()
            }
        }
    }
}

#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum Priority {
    Idle = 0,
    Low,
    Normal,
    High,
    Realtime,
}

impl Priority {
    pub fn is_useful(self) -> bool {
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

impl Quantum {
    const fn new(value: u8) -> Self {
        Quantum {
            current: value,
            default: value,
        }
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.current = self.default;
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

#[derive(Default)]
struct ThreadPool {
    data: BTreeMap<ThreadHandle, Box<RawThread>>,
    lock: Spinlock,
}

impl ThreadPool {
    #[inline]
    #[track_caller]
    fn synchronized<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        unsafe {
            Cpu::without_interrupts(|| {
                let shared = Self::shared();
                shared.lock.synchronized(f)
            })
        }
    }

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a mut Self {
        &mut MyScheduler::shared().pool
    }

    fn add(thread: Box<RawThread>) {
        Self::synchronized(|| {
            let shared = Self::shared();
            let handle = thread.handle;
            shared.data.insert(handle, thread);
        });
    }

    fn drop_thread(handle: ThreadHandle) {
        Self::synchronized(|| {
            let shared = Self::shared();
            shared.data.remove(&handle);
        });
    }

    fn get(&self, key: &ThreadHandle) -> Option<&Box<RawThread>> {
        Self::synchronized(|| self.data.get(key))
    }

    fn get_mut(&mut self, key: &ThreadHandle) -> Option<&mut Box<RawThread>> {
        Self::synchronized(move || self.data.get_mut(key))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ThreadHandle(NonZeroUsize);

impl ThreadHandle {
    #[inline]
    fn new(val: usize) -> Option<Self> {
        NonZeroUsize::new(val).map(|x| Self(x))
    }

    /// Acquire the next thread ID
    #[inline]
    fn next() -> ThreadHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        ThreadHandle::new(NEXT_ID.fetch_add(1, Ordering::Relaxed)).unwrap()
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0.get()
    }

    #[inline]
    #[track_caller]
    fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RawThread) -> R,
    {
        let shared = ThreadPool::shared();
        let thread = shared.get_mut(self).unwrap();
        f(thread)
    }

    #[inline]
    fn get<'a>(&self) -> Option<&'a Box<RawThread>> {
        let shared = ThreadPool::shared();
        shared.get(self)
    }

    #[inline]
    #[track_caller]
    fn as_ref<'a>(&self) -> &'a RawThread {
        self.get().unwrap()
    }

    #[inline]
    pub fn name(&self) -> Option<&str> {
        self.get().and_then(|v| v.name())
    }

    #[inline]
    fn wake(&self) {
        self.as_ref().attribute.insert(ThreadAttributes::AWAKE);
        MyScheduler::add(*self);
    }

    #[inline]
    pub fn join(&self) -> usize {
        self.get().map(|t| t.sem.wait());
        0
    }
}

const SIZE_OF_CONTEXT: usize = 512;
const SIZE_OF_STACK: usize = 0x10000;
const THREAD_NAME_LENGTH: usize = 32;

type ThreadStart = fn(usize) -> ();

#[allow(dead_code)]
struct RawThread {
    /// Architectural context data
    context: [u8; SIZE_OF_CONTEXT],

    /// IDs
    pid: ProcessId,
    handle: ThreadHandle,

    // Properties
    sem: Semaphore,
    personality: Option<Box<dyn Personality>>,
    attribute: ThreadAttributes,
    priority: Priority,
    quantum: Quantum,

    // Timer supports (deprecated)
    deadline: Timer,

    // Statistics
    measure: AtomicU64,
    cpu_time: AtomicU64,
    load0: AtomicU32,
    load: AtomicU32,

    // Executor
    executor: Option<Executor>,

    /// Thread Name
    name: [u8; THREAD_NAME_LENGTH],
}

#[derive(Default)]
struct ThreadAttributes(AtomicUsize);

#[allow(dead_code)]
impl ThreadAttributes {
    pub const EMPTY: Self = Self::new(0);
    pub const QUEUED: usize = 0b0000_0000_0000_0001;
    pub const ASLEEP: usize = 0b0000_0000_0000_0010;
    pub const AWAKE: usize = 0b0000_0000_0000_0100;
    pub const ZOMBIE: usize = 0b0000_0000_0000_1000;

    #[inline]
    pub const fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }

    #[inline]
    pub fn contains(&self, bits: usize) -> bool {
        (self.0.load(Ordering::Relaxed) & bits) == bits
    }

    #[inline]
    pub fn insert(&self, bits: usize) {
        self.0.fetch_or(bits, Ordering::SeqCst);
    }

    #[inline]
    pub fn remove(&self, bits: usize) {
        self.0.fetch_and(!bits, Ordering::SeqCst);
    }

    #[inline]
    fn test_and_set(&self, bits: usize) -> bool {
        (self.0.fetch_or(bits, Ordering::SeqCst) & bits) == bits
    }

    #[inline]
    fn test_and_clear(&self, bits: usize) -> bool {
        (self.0.fetch_and(!bits, Ordering::SeqCst) & bits) == bits
    }

    fn to_char(&self) -> char {
        if self.contains(Self::ZOMBIE) {
            'Z'
        } else if self.contains(Self::AWAKE) {
            'W'
        } else if self.contains(Self::ASLEEP) {
            'S'
        } else if self.contains(Self::QUEUED) {
            'R'
        } else {
            '-'
        }
    }
}

use core::fmt;
impl fmt::Display for ThreadAttributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

#[allow(dead_code)]
impl RawThread {
    fn new(
        pid: ProcessId,
        priority: Priority,
        name: &str,
        start: Option<ThreadStart>,
        args: usize,
        personality: Option<Box<dyn Personality>>,
    ) -> ThreadHandle {
        let handle = ThreadHandle::next();
        let mut thread = Self {
            context: [0; SIZE_OF_CONTEXT],
            pid,
            handle,
            sem: Semaphore::new(0),
            attribute: ThreadAttributes::EMPTY,
            priority,
            quantum: Quantum::from(priority),
            deadline: Timer::JUST,
            measure: AtomicU64::new(0),
            cpu_time: AtomicU64::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            executor: None,
            personality,
            name: [0; THREAD_NAME_LENGTH],
        };
        if let Some(start) = start {
            unsafe {
                let stack = MemoryManager::zalloc(SIZE_OF_STACK).unwrap().get() as *mut c_void;
                asm_sch_make_new_thread(
                    thread.context.as_mut_ptr(),
                    stack.add(SIZE_OF_STACK),
                    start as usize,
                    args,
                );
            }
        }
        thread.set_name(name);
        ThreadPool::add(Box::new(thread));
        handle
    }

    fn exit(&mut self) -> ! {
        self.sem.signal();
        self.personality
            .as_mut()
            .map(|personality| personality.on_exit());

        self.attribute.insert(ThreadAttributes::ZOMBIE);
        MyScheduler::sleep();
        unreachable!();
    }

    fn set_name_array(array: &mut [u8; THREAD_NAME_LENGTH], name: &str) {
        let mut i = 1;
        for c in name.bytes() {
            if i >= THREAD_NAME_LENGTH {
                break;
            }
            array[i] = c;
            i += 1;
        }
        array[0] = i as u8 - 1;
    }

    fn set_name(&mut self, name: &str) {
        RawThread::set_name_array(&mut self.name, name);
    }

    fn name<'a>(&self) -> Option<&'a str> {
        let len = self.name[0] as usize;
        match len {
            0 => None,
            _ => core::str::from_utf8(unsafe { core::slice::from_raw_parts(&self.name[1], len) })
                .ok(),
        }
    }
}

#[derive(Debug)]
pub struct SignallingObject(AtomicUsize);

impl SignallingObject {
    const NONE: usize = 0;

    pub fn new() -> Self {
        Self(AtomicUsize::new(
            MyScheduler::current_thread().unwrap().as_usize(),
        ))
    }

    pub fn set(&self, value: ThreadHandle) -> Result<(), ()> {
        let value = value.as_usize();
        match self
            .0
            .compare_exchange(Self::NONE, value, Ordering::SeqCst, Ordering::Relaxed)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn load(&self) -> Option<ThreadHandle> {
        ThreadHandle::new(self.0.load(Ordering::SeqCst))
    }

    pub fn unbox(&self) -> Option<ThreadHandle> {
        ThreadHandle::new(self.0.swap(Self::NONE, Ordering::SeqCst))
    }

    pub fn wait(&self, duration: Duration) {
        MyScheduler::wait_for(Some(self), duration)
    }

    pub fn signal(&self) {
        if let Some(thread) = self.unbox() {
            thread.wake()
        }
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
    fn with_capacity(capacity: usize) -> Self {
        Self(ArrayQueue::new(capacity))
    }
    fn dequeue(&self) -> Option<ThreadHandle> {
        self.0.pop().ok().map(|v| ThreadHandle(v))
    }
    fn enqueue(&self, data: ThreadHandle) -> Result<(), ()> {
        self.0.push(data.0).map_err(|_| ())
    }
}
