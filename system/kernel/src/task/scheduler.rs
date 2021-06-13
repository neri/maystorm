//! Scheduler

use super::{executor::Executor, *};
use crate::{
    arch::cpu::*,
    rt::Personality,
    sync::{atomicflags::*, semaphore::*, spinlock::*, Mutex},
    system::*,
    ui::window::{WindowHandle, WindowMessage},
    *,
};
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::*};
use bitflags::*;
use core::{
    cell::UnsafeCell, ffi::c_void, fmt::Write, num::*, ops::*, sync::atomic::*, time::Duration,
};
use crossbeam_queue::ArrayQueue;
use megstd::string::*;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

// const THRESHOLD_SAVING: usize = 666;
// const THRESHOLD_FULL_THROTTLE_MODE: usize = 750;

static mut SCHEDULER: Option<Box<Scheduler>> = None;

static SCHEDULER_ENABLED: AtomicBool = AtomicBool::new(false);

/// System Scheduler
pub struct Scheduler {
    queue_realtime: ThreadQueue,
    queue_higher: ThreadQueue,
    queue_normal: ThreadQueue,

    locals: Vec<Box<LocalScheduler>>,

    process_pool: ProcessPool,
    thread_pool: ThreadPool,

    usage: AtomicUsize,
    usage_total: AtomicUsize,
    is_frozen: AtomicBool,
    state: SchedulerState,

    next_timer: AtomicUsize,
    sem_timer: Semaphore,
    timer_queue: ArrayQueue<TimerEvent>,
}

#[allow(non_camel_case_types)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum SchedulerState {
    /// The scheduler has not started yet.
    Disabled = 0,
    /// The scheduler is running on minimal power.
    Saving,
    /// The scheduler is running.
    Running,
    /// The scheduler is running on maximum power.
    FullThrottle,
    MAX,
}

impl Scheduler {
    /// Start scheduler and sleep forever
    pub(crate) fn start(f: fn(usize) -> (), args: usize) -> ! {
        const SIZE_OF_SUB_QUEUE: usize = 63;
        const SIZE_OF_MAIN_QUEUE: usize = 255;

        let queue_realtime = ThreadQueue::with_capacity(SIZE_OF_SUB_QUEUE);
        let queue_higher = ThreadQueue::with_capacity(SIZE_OF_SUB_QUEUE);
        let queue_normal = ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE);

        unsafe {
            SCHEDULER = Some(Box::new(Self {
                queue_realtime,
                queue_higher,
                queue_normal,
                locals: Vec::new(),
                process_pool: ProcessPool::default(),
                thread_pool: ThreadPool::default(),
                usage: AtomicUsize::new(0),
                usage_total: AtomicUsize::new(0),
                is_frozen: AtomicBool::new(false),
                state: SchedulerState::Running,
                next_timer: AtomicUsize::new(0),
                sem_timer: Semaphore::new(0),
                timer_queue: ArrayQueue::new(100),
            }));
        }

        ProcessPool::add(ProcessContextData::new(
            ProcessId(0),
            Priority::Idle,
            "idle",
        ));

        let sch = Self::shared();
        for index in 0..System::current_device().num_of_active_cpus() {
            sch.locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        SpawnOption::with_priority(Priority::Normal).start_process(f, args, "System");

        SpawnOption::with_priority(Priority::Realtime).start_process(
            Self::scheduler_thread,
            0,
            "Scheduler",
        );

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

    /// Returns whether or not the thread scheduler is running.
    pub fn is_enabled() -> bool {
        unsafe { &SCHEDULER }.is_some() && SCHEDULER_ENABLED.load(Ordering::SeqCst)
    }

    /// Returns the current state of the scheduler.
    pub fn current_state() -> SchedulerState {
        if Self::is_enabled() {
            Self::shared().state
        } else {
            SchedulerState::Disabled
        }
    }

    /// All threads will stop.
    pub unsafe fn freeze(force: bool) -> Result<(), ()> {
        let sch = Self::shared();
        sch.is_frozen.store(true, Ordering::SeqCst);
        if force {
            let _ = Cpu::broadcast_schedule();
        }
        Ok(())
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
        unsafe { without_interrupts!(Self::local_scheduler().map(|sch| sch.current_thread())) }
    }

    /// Get the personality instance associated with the current thread
    #[inline]
    pub fn current_personality<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&mut Box<dyn Personality>) -> R,
    {
        Self::current_thread()
            .and_then(|thread| unsafe { thread.unsafe_weak() })
            .and_then(|thread| thread.personality.as_mut())
            .map(|v| f(v))
    }

    /// Perform the preemption
    pub unsafe fn reschedule() {
        if !Self::is_enabled() {
            return;
        }
        let local = Self::local_scheduler().unwrap();
        let current = local.current_thread();
        current.update_statistics();
        let priority = { current.as_ref().priority };
        let shared = Self::shared();
        if !Timer::from_usize(shared.next_timer.load(Ordering::SeqCst)).until() {
            shared.sem_timer.signal();
        }
        if priority == Priority::Realtime {
            return;
        }
        if let Some(next) = shared.queue_realtime.dequeue() {
            LocalScheduler::switch_context(local, next);
        } else if let Some(next) = (priority < Priority::High)
            .then(|| shared.queue_higher.dequeue())
            .flatten()
        {
            LocalScheduler::switch_context(local, next);
        } else if let Some(next) = (priority < Priority::Normal)
            .then(|| shared.queue_normal.dequeue())
            .flatten()
        {
            LocalScheduler::switch_context(local, next);
        } else if current.update(|current| current.quantum.consume()) {
            if let Some(next) = Scheduler::next(local) {
                LocalScheduler::switch_context(local, next);
            }
        }
    }

    pub fn sleep() {
        unsafe {
            without_interrupts!({
                let local = Self::local_scheduler().unwrap();
                let current = local.current_thread();
                current.update_statistics();
                current.as_ref().attribute.insert(ThreadAttributes::ASLEEP);
                LocalScheduler::switch_context(local, Scheduler::next(local).unwrap_or(local.idle));
            });
        }
    }

    fn yield_thread() {
        unsafe {
            without_interrupts!({
                let local = Self::local_scheduler().unwrap();
                local.current_thread().update_statistics();
                LocalScheduler::switch_context(local, Scheduler::next(local).unwrap_or(local.idle));
            });
        }
    }

    /// Get the scheduler for the current processor
    #[inline]
    unsafe fn local_scheduler() -> Option<&'static mut Box<LocalScheduler>> {
        match SCHEDULER.as_mut() {
            Some(sch) => sch.locals.get_mut(Cpu::current_processor_index().0),
            None => None,
        }
    }

    /// Get the next executable thread from the thread queue
    fn next(scheduler: &LocalScheduler) -> Option<ThreadHandle> {
        let shared = Self::shared();
        if shared.is_frozen.load(Ordering::SeqCst) {
            Some(scheduler.idle)
        } else if let Some(next) = shared.queue_realtime.dequeue() {
            Some(next)
        } else if let Some(next) = shared.queue_higher.dequeue() {
            Some(next)
        } else if let Some(next) = shared.queue_normal.dequeue() {
            Some(next)
        } else {
            None
        }
    }

    fn enqueue(&mut self, handle: ThreadHandle) {
        match handle.as_ref().priority {
            Priority::Realtime => self.queue_realtime.enqueue(handle).unwrap(),
            Priority::High | Priority::Normal | Priority::Low => {
                self.queue_normal.enqueue(handle).unwrap()
            }
            _ => unreachable!(),
        }
    }

    /// Retire Thread
    fn retire(thread: ThreadHandle) {
        let handle = thread;
        let shared = Self::shared();
        let thread = handle.as_ref();
        if thread.priority == Priority::Idle {
            return;
        } else if thread.attribute.contains(ThreadAttributes::ZOMBIE) {
            ThreadPool::remove(handle);
        } else if thread.attribute.test_and_clear(ThreadAttributes::AWAKE) {
            thread.attribute.remove(ThreadAttributes::ASLEEP);
            shared.enqueue(handle);
        } else if thread.attribute.contains(ThreadAttributes::ASLEEP) {
            thread.attribute.remove(ThreadAttributes::QUEUED);
        } else {
            shared.enqueue(handle);
        }
    }

    /// Add thread to the queue
    fn add(thread: ThreadHandle) {
        let handle = thread;
        let shared = Self::shared();
        let thread = handle.as_ref();
        if thread.priority == Priority::Idle || thread.attribute.contains(ThreadAttributes::ZOMBIE)
        {
            return;
        }
        if !thread.attribute.test_and_set(ThreadAttributes::QUEUED) {
            if thread.attribute.test_and_clear(ThreadAttributes::AWAKE) {
                thread.attribute.remove(ThreadAttributes::ASLEEP);
            }
            shared.enqueue(handle);
        }
    }

    /// Schedule a timer event
    pub fn schedule_timer(event: TimerEvent) -> Result<(), TimerEvent> {
        let shared = Self::shared();
        shared
            .timer_queue
            .push(event)
            .map(|_| shared.sem_timer.signal())
    }

    /// Scheduler
    fn scheduler_thread(_args: usize) {
        let shared = Self::shared();

        SpawnOption::with_priority(Priority::Realtime).start(
            Self::statistics_thread,
            0,
            "Statistics",
        );

        let mut events: Vec<TimerEvent> = Vec::with_capacity(100);
        loop {
            if let Some(event) = shared.timer_queue.pop() {
                events.push(event);
                while let Some(event) = shared.timer_queue.pop() {
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
                shared
                    .next_timer
                    .store(event.timer.into_usize(), Ordering::SeqCst);
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
            let actual = now.0 - measure.0;
            let actual1000 = actual as usize * 1000;

            let mut usage = 0;
            for thread in shared.thread_pool.data.values() {
                let thread = thread.clone();
                let thread = unsafe { &mut (*thread.get()) };

                let load0 = thread.load0.swap(0, Ordering::SeqCst);
                let load = usize::min(load0 as usize * expect as usize / actual1000, 1000);
                thread.load.store(load as u32, Ordering::SeqCst);
                if thread.priority != Priority::Idle {
                    usage += load;
                }

                let process = thread.pid.get().unwrap();
                process.cpu_time.fetch_add(load0 as usize, Ordering::SeqCst);
                process.load0.fetch_add(load as u32, Ordering::SeqCst);
            }

            for process in shared.process_pool.data.values() {
                let process = process.clone();
                let process = unsafe { &mut (*process.get()) };
                let load = process.load0.swap(0, Ordering::SeqCst);
                process.load.store(load, Ordering::SeqCst);
            }

            let num_cpu = System::current_device().num_of_active_cpus();
            let usage_total = usize::min(usage, num_cpu * 1000);
            let usage_per_cpu = usize::min(usage / num_cpu, 1000);
            shared.usage_total.store(usage_total, Ordering::SeqCst);
            shared.usage.store(usage_per_cpu, Ordering::SeqCst);

            // if usage_total < THRESHOLD_SAVING {
            //     shared.state = SchedulerState::Saving;
            // } else if usage_total
            //     > System::current_device().num_of_performance_cpus() * THRESHOLD_FULL_THROTTLE_MODE
            // {
            //     shared.state = SchedulerState::FullThrottle;
            // } else {
            //     shared.state = SchedulerState::Running;
            // }

            measure = now;
        }
    }

    #[inline]
    pub fn usage_per_cpu() -> usize {
        let shared = Self::shared();
        shared.usage.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn usage_total() -> usize {
        let shared = Self::shared();
        shared.usage_total.load(Ordering::Relaxed)
    }

    #[track_caller]
    fn spawn(
        start: ThreadStart,
        args: usize,
        name: &str,
        options: SpawnOption,
    ) -> Option<ThreadHandle> {
        let current_pid = Self::current_pid().unwrap_or(ProcessId(0));
        let pid = if options.raise_pid {
            let child =
                ProcessContextData::new(current_pid, options.priority.unwrap_or_default(), name);
            let pid = child.pid;
            ProcessPool::add(child);
            pid
        } else {
            current_pid
        };
        let target_process = pid.get().unwrap();
        let priority = match options.priority {
            Some(v) => v,
            None => target_process.priority,
        };
        target_process.n_threads.fetch_add(1, Ordering::SeqCst);
        let thread =
            ThreadContextData::new(pid, priority, name, Some(start), args, options.personality);
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
        let current = Self::current_thread().unwrap();
        unsafe {
            current.unsafe_weak().unwrap().exit();
        }
    }

    pub fn get_idle_statistics(vec: &mut Vec<u32>) {
        let sch = Self::shared();
        vec.clear();
        for thread in sch.thread_pool.data.values() {
            let thread = thread.clone();
            let thread = unsafe { &(*thread.get()) };
            if thread.priority != Priority::Idle {
                break;
            }
            vec.push(thread.load.load(Ordering::Relaxed));
        }
    }

    pub fn print_statistics(sb: &mut StringBuffer, _exclude_idle: bool) {
        let max_load = 1000 * System::current_device().num_of_active_cpus() as u32;
        let sch = Self::shared();
        writeln!(sb, "PID P #TH %CPU TIME     NAME").unwrap();
        for process in sch.process_pool.data.values() {
            let process = process.clone();
            let process = unsafe { &*process.get() };
            if process.pid == ProcessId(0) {
                continue;
            }

            write!(
                sb,
                "{:3} {} {:3}",
                process.pid.0,
                process.priority as usize,
                process.n_threads.load(Ordering::Relaxed),
            )
            .unwrap();

            let load = u32::min(process.load.load(Ordering::Relaxed), max_load);
            let load0 = load % 10;
            let load1 = load / 10;
            if load1 >= 100 {
                write!(sb, " {:4}", load1,).unwrap();
            } else {
                write!(sb, " {:2}.{:1}", load1, load0,).unwrap();
            }

            let time = process.cpu_time.load(Ordering::Relaxed) / 10_000;
            let dsec = time % 100;
            let sec = time / 100 % 60;
            let min = time / 60_00 % 60;
            let hour = time / 3600_00;
            if hour > 0 {
                write!(sb, " {:02}:{:02}:{:02}", hour, min, sec,).unwrap();
            } else {
                write!(sb, " {:02}:{:02}.{:02}", min, sec, dsec,).unwrap();
            }

            match process.name() {
                Some(name) => writeln!(sb, " {}", name,).unwrap(),
                None => (),
            }
        }
    }
}

/// Processor Local Scheduler
#[allow(dead_code)]
struct LocalScheduler {
    index: ProcessorIndex,
    idle: ThreadHandle,
    current: AtomicUsize,
    retired: AtomicUsize,
    irql: AtomicUsize,
}

impl LocalScheduler {
    fn new(index: ProcessorIndex) -> Box<Self> {
        let mut sb = Sb255::new();
        write!(sb, "idle.{}", index.0).unwrap();
        let idle = ThreadContextData::new(ProcessId(0), Priority::Idle, sb.as_str(), None, 0, None);
        Box::new(Self {
            index,
            idle,
            current: AtomicUsize::new(idle.as_usize()),
            retired: AtomicUsize::new(0),
            irql: AtomicUsize::new(0),
        })
    }

    #[inline(never)]
    unsafe fn switch_context(scheduler: &'static mut Self, next: ThreadHandle) {
        scheduler._switch_context(next);
    }

    #[inline]
    unsafe fn _switch_context(&mut self, next: ThreadHandle) {
        let old_irql = self.raise_irql(Irql::Dispatch);
        let current = self.current_thread();
        if current.as_ref().handle != next.as_ref().handle {
            self.swap_retired(Some(current));
            self.current.store(next.as_usize(), Ordering::SeqCst);

            {
                let current = current.unsafe_weak().unwrap();
                let next = &next.unsafe_weak().unwrap().context;
                current.context.switch(next);
            }

            Scheduler::local_scheduler()
                .unwrap()
                ._switch_context_after(old_irql);
        } else {
            self.lower_irql(old_irql);
        }
    }

    #[inline]
    unsafe fn _switch_context_after(&mut self, irql: Irql) {
        let current = self.current_thread();

        current.update(|thread| {
            thread.attribute.remove(ThreadAttributes::AWAKE);
            thread.attribute.remove(ThreadAttributes::ASLEEP);
            thread.measure.store(Timer::measure().0, Ordering::SeqCst);
        });
        let retired = self.swap_retired(None).unwrap();
        Scheduler::retire(retired);
        self.lower_irql(irql);
    }

    #[inline]
    fn swap_retired(&self, val: Option<ThreadHandle>) -> Option<ThreadHandle> {
        ThreadHandle::new(
            self.retired
                .swap(val.map(|v| v.as_usize()).unwrap_or(0), Ordering::SeqCst),
        )
    }

    #[inline]
    fn current_thread(&self) -> ThreadHandle {
        unsafe { ThreadHandle::new_unchecked(self.current.load(Ordering::SeqCst)) }
    }

    #[inline]
    fn current_irql(&self) -> Irql {
        FromPrimitive::from_usize(self.irql.load(Ordering::SeqCst)).unwrap_or(Irql::Passive)
    }

    #[inline]
    #[track_caller]
    unsafe fn raise_irql(&self, new_irql: Irql) -> Irql {
        let old_irql = self.current_irql();
        if new_irql < old_irql {
            panic!("IRQL_NOT_GREATER_OR_EQUAL");
        }
        self.irql.store(new_irql as usize, Ordering::SeqCst);
        old_irql
    }

    #[inline]
    #[track_caller]
    unsafe fn lower_irql(&self, new_irql: Irql) {
        let old_irql = self.current_irql();
        if new_irql > old_irql {
            panic!("IRQL_NOT_LESS_OR_EQUAL");
        }
        self.irql.store(new_irql as usize, Ordering::SeqCst);
    }
}

#[no_mangle]
pub unsafe extern "C" fn sch_setup_new_thread() {
    let lsch = Scheduler::local_scheduler().unwrap();
    let current = lsch.current_thread();
    current.update(|thread| {
        thread.measure.store(Timer::measure().0, Ordering::SeqCst);
    });
    let retired = lsch.swap_retired(None).unwrap();
    Scheduler::retire(retired);
    lsch.lower_irql(Irql::Passive);
}

/// Build an option to start a new thread or process.
pub struct SpawnOption {
    priority: Option<Priority>,
    raise_pid: bool,
    personality: Option<Box<dyn Personality>>,
}

impl SpawnOption {
    #[inline]
    pub const fn new() -> Self {
        Self {
            priority: None,
            raise_pid: false,
            personality: None,
        }
    }

    #[inline]
    pub const fn with_priority(priority: Priority) -> Self {
        Self {
            priority: Some(priority),
            raise_pid: false,
            personality: None,
        }
    }

    #[inline]
    pub fn personality(mut self, personality: Box<dyn Personality>) -> Self {
        self.personality = Some(personality);
        self
    }

    /// Start the specified function in a new thread.
    #[inline]
    pub fn start(self, start: fn(usize), args: usize, name: &str) -> Option<ThreadHandle> {
        Scheduler::spawn(start, args, name, self)
    }

    /// Start the specified function in a new process.
    #[inline]
    pub fn start_process(mut self, start: fn(usize), args: usize, name: &str) -> Option<ProcessId> {
        self.raise_pid = true;
        Scheduler::spawn(start, args, name, self)
            .and_then(|v| v.get())
            .map(|v| v.pid)
    }

    /// Start the closure in a new thread.
    ///
    /// The parameters passed follow the move semantics of Rust's closure.
    #[inline]
    pub fn spawn<F, T>(self, start: F, name: &str) -> JoinHandle<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        FnSpawner::spawn(start, name, self)
    }
}

/// Wrapper object to spawn the closure
struct FnSpawner<F, T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    start: F,
    mutex: Arc<Mutex<Option<T>>>,
}

impl<F, T> FnSpawner<F, T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    fn spawn(start: F, name: &str, options: SpawnOption) -> JoinHandle<T> {
        let mutex = Arc::new(Mutex::new(None));
        let boxed = Arc::new(Box::new(Self {
            start,
            mutex: Arc::clone(&mutex),
        }));
        let thread = unsafe {
            let ptr = Arc::into_raw(boxed);
            Arc::increment_strong_count(ptr);
            Scheduler::spawn(Self::start_thread, ptr as usize, name, options)
        }
        .unwrap();

        JoinHandle { thread, mutex }
    }

    fn start_thread(p: usize) {
        unsafe {
            let ptr = p as *const Box<Self>;
            let p = Arc::from_raw(ptr);
            Arc::decrement_strong_count(ptr);
            let p = match Arc::try_unwrap(p) {
                Ok(p) => p,
                Err(_) => unreachable!(),
            };
            let p = Box::into_inner(p);
            let r = (p.start)();
            *p.mutex.lock().unwrap() = Some(r);
        };
        Scheduler::exit();
    }
}

pub struct JoinHandle<T> {
    thread: ThreadHandle,
    mutex: Arc<Mutex<Option<T>>>,
}

impl<T> JoinHandle<T> {
    // pub fn thread(&self) -> &Thread

    pub fn join(self) -> Result<T, ()> {
        self.thread.join();

        match Arc::try_unwrap(self.mutex) {
            Ok(v) => {
                let t = v.into_inner().unwrap();
                t.ok_or(())
            }
            Err(_) => unreachable!(),
        }
    }
}

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub trait TimerSource {
    fn measure(&self) -> TimeSpec;

    fn from_duration(&self, val: Duration) -> TimeSpec;

    fn to_duration(&self, val: TimeSpec) -> Duration;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Timer {
    deadline: TimeSpec,
}

impl Timer {
    pub const JUST: Timer = Timer {
        deadline: TimeSpec(0),
    };

    #[inline]
    pub const fn from_usize(val: usize) -> Self {
        Self {
            deadline: TimeSpec(val),
        }
    }

    #[inline]
    pub const fn into_usize(&self) -> usize {
        self.deadline.0
    }

    #[inline]
    pub fn new(duration: Duration) -> Self {
        let timer = Self::timer_source();
        Timer {
            deadline: timer.measure() + duration.into(),
        }
    }

    #[inline]
    pub fn epsilon() -> Self {
        let timer = Self::timer_source();
        Timer {
            deadline: timer.measure() + TimeSpec::EPSILON,
        }
    }

    #[inline]
    pub const fn is_just(&self) -> bool {
        self.deadline.0 == 0
    }

    #[inline]
    pub fn until(&self) -> bool {
        if self.is_just() {
            false
        } else {
            let timer = Self::timer_source();
            self.deadline > timer.measure()
        }
    }

    #[inline]
    pub fn repeat_until<F>(&self, mut f: F)
    where
        F: FnMut(),
    {
        while self.until() {
            f()
        }
    }

    #[inline]
    pub unsafe fn set_timer(source: Box<dyn TimerSource>) {
        TIMER_SOURCE = Some(source);
    }

    fn timer_source<'a>() -> &'a Box<dyn TimerSource> {
        unsafe { TIMER_SOURCE.as_ref().unwrap() }
    }

    #[track_caller]
    pub fn sleep(duration: Duration) {
        if Scheduler::is_enabled() {
            let timer = Timer::new(duration);
            let mut event = TimerEvent::one_shot(timer);
            while timer.until() {
                match Scheduler::schedule_timer(event) {
                    Ok(()) => {
                        Scheduler::sleep();
                        return;
                    }
                    Err(e) => {
                        event = e;
                        Scheduler::yield_thread();
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
    pub fn measure() -> TimeSpec {
        Self::timer_source().measure()
    }

    #[inline]
    pub fn monotonic() -> Duration {
        Self::measure().into()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpec(pub usize);

impl TimeSpec {
    pub const EPSILON: Self = Self(1);

    #[inline]
    fn into_duration(&self) -> Duration {
        Timer::timer_source().to_duration(*self)
    }

    #[inline]
    fn from_duration(val: Duration) -> TimeSpec {
        Timer::timer_source().from_duration(val)
    }
}

impl Add<TimeSpec> for TimeSpec {
    type Output = Self;
    #[inline]
    fn add(self, rhs: TimeSpec) -> Self::Output {
        TimeSpec(self.0 + rhs.0)
    }
}

impl From<TimeSpec> for Duration {
    #[inline]
    fn from(val: TimeSpec) -> Duration {
        val.into_duration()
    }
}

impl From<Duration> for TimeSpec {
    #[inline]
    fn from(val: Duration) -> TimeSpec {
        TimeSpec::from_duration(val)
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
            timer_type: TimerType::OneShot(Scheduler::current_thread().unwrap()),
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

/// Thread Priority
#[repr(u8)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum Priority {
    /// This is the lowest priority at which the processor will be idle when all other threads are waiting. This will never be scheduled.
    Idle = 0,
    /// Lower than normal proirity
    Low,
    /// This is the normal priority that is scheduled in a round-robin fashion.
    /// When the allocated quanta are consumed, they are preempted.
    Normal,
    /// Higher than normal priority
    High,
    /// Currently, the highest priority and will not be preempted.
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

impl Default for Priority {
    #[inline]
    fn default() -> Self {
        Self::Normal
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
pub struct ProcessPool {
    data: BTreeMap<ProcessId, Arc<UnsafeCell<Box<ProcessContextData>>>>,
    lock: Spinlock,
}

impl ProcessPool {
    #[inline]
    #[track_caller]
    fn synchronized<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        unsafe {
            without_interrupts!({
                let shared = Self::shared();
                shared.lock.synchronized(f)
            })
        }
    }

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a mut Self {
        &mut Scheduler::shared().process_pool
    }

    fn add(process: Box<ProcessContextData>) {
        Self::synchronized(|| {
            let shared = Self::shared();
            let key = process.pid;
            shared.data.insert(key, Arc::new(UnsafeCell::new(process)));
        });
    }

    #[inline]
    fn remove(pid: ProcessId) {
        Self::synchronized(|| {
            let shared = Self::shared();
            shared.data.remove(&pid);
        });
    }

    #[inline]
    unsafe fn unsafe_weak<'a>(&self, key: ProcessId) -> Option<&'a mut Box<ProcessContextData>> {
        Self::synchronized(|| self.data.get(&key).map(|v| &mut *(&*Arc::as_ptr(v)).get()))
    }

    #[inline]
    fn get<'a>(&self, key: ProcessId) -> Option<&'a Box<ProcessContextData>> {
        Self::synchronized(|| self.data.get(&key).map(|v| v.clone().get()))
            .map(|thread| unsafe { &(*thread) })
    }

    #[inline]
    fn get_mut<F, R>(&mut self, key: ProcessId, f: F) -> Option<R>
    where
        F: FnOnce(&mut ProcessContextData) -> R,
    {
        Self::synchronized(move || self.data.get_mut(&key).map(|v| v.clone())).map(
            |process| unsafe {
                let process = process.get();
                f(&mut *process)
            },
        )
    }
}

#[derive(Default)]
struct ThreadPool {
    data: BTreeMap<ThreadHandle, Arc<UnsafeCell<Box<ThreadContextData>>>>,
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
            without_interrupts!({
                let shared = Self::shared();
                shared.lock.synchronized(f)
            })
        }
    }

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a mut Self {
        &mut Scheduler::shared().thread_pool
    }

    fn add(thread: Box<ThreadContextData>) {
        Self::synchronized(|| {
            let shared = Self::shared();
            let handle = thread.handle;
            shared
                .data
                .insert(handle, Arc::new(UnsafeCell::new(thread)));
        });
    }

    #[inline]
    fn remove(handle: ThreadHandle) {
        Self::synchronized(|| {
            let shared = Self::shared();
            shared.data.remove(&handle);
        });
    }

    #[inline]
    unsafe fn unsafe_weak<'a>(&self, key: ThreadHandle) -> Option<&'a mut Box<ThreadContextData>> {
        Self::synchronized(|| self.data.get(&key).map(|v| &mut *(&*Arc::as_ptr(v)).get()))
    }

    #[inline]
    fn get<'a>(&self, key: ThreadHandle) -> Option<&'a Box<ThreadContextData>> {
        Self::synchronized(|| self.data.get(&key).map(|v| v.clone().get()))
            .map(|thread| unsafe { &(*thread) })
    }

    #[inline]
    fn get_mut<F, R>(&mut self, key: ThreadHandle, f: F) -> Option<R>
    where
        F: FnOnce(&mut ThreadContextData) -> R,
    {
        Self::synchronized(move || self.data.get_mut(&key).map(|v| v.clone())).map(
            |thread| unsafe {
                let thread = thread.get();
                f(&mut *thread)
            },
        )
    }
}

#[repr(transparent)]
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ProcessId(pub usize);

impl ProcessId {
    #[inline]
    fn get<'a>(&self) -> Option<&'a Box<ProcessContextData>> {
        let shared = ProcessPool::shared();
        shared.get(*self)
    }

    #[inline]
    pub fn join(&self) {
        self.get().map(|t| t.sem.wait());
    }
}

struct ProcessContextData {
    parent: ProcessId,
    pid: ProcessId,
    n_threads: AtomicUsize,
    priority: Priority,
    sem: Semaphore,

    start_time: TimeSpec,
    cpu_time: AtomicUsize,
    load0: AtomicU32,
    load: AtomicU32,

    name: [u8; CONTEXT_LABEL_LENGTH],
}

const CONTEXT_LABEL_LENGTH: usize = 32;

fn set_name_array(array: &mut [u8; CONTEXT_LABEL_LENGTH], name: &str) {
    let mut i = 1;
    for c in name.bytes() {
        if i >= CONTEXT_LABEL_LENGTH {
            break;
        }
        array[i] = c;
        i += 1;
    }
    array[0] = i as u8 - 1;
}

impl ProcessContextData {
    fn new(parent: ProcessId, priority: Priority, name: &str) -> Box<ProcessContextData> {
        let pid = Self::next_pid();
        let mut child = Self {
            parent,
            pid,
            n_threads: AtomicUsize::new(0),
            priority,
            sem: Semaphore::new(0),
            start_time: Timer::monotonic().into(),
            cpu_time: AtomicUsize::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            name: [0u8; CONTEXT_LABEL_LENGTH],
        };

        set_name_array(&mut child.name, name);

        Box::new(child)
    }

    #[inline]
    fn next_pid() -> ProcessId {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
        ProcessId(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }

    fn set_name(&mut self, name: &str) {
        set_name_array(&mut self.name, name);
    }

    fn name<'a>(&self) -> Option<&'a str> {
        let len = self.name[0] as usize;
        match len {
            0 => None,
            _ => core::str::from_utf8(unsafe { core::slice::from_raw_parts(&self.name[1], len) })
                .ok(),
        }
    }

    fn exit(&self) {
        self.sem.signal();
        ProcessPool::remove(self.pid);
    }
}

impl Drop for ProcessContextData {
    fn drop(&mut self) {
        println!("drop {}", self.pid.0);
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ThreadHandle(NonZeroUsize);

impl ThreadHandle {
    #[inline]
    pub const fn new(val: usize) -> Option<Self> {
        match NonZeroUsize::new(val) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    const unsafe fn new_unchecked(val: usize) -> Self {
        Self(NonZeroUsize::new_unchecked(val))
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
        F: FnOnce(&mut ThreadContextData) -> R,
    {
        let shared = ThreadPool::shared();
        shared.get_mut(*self, f).unwrap()
    }

    #[inline]
    fn get<'a>(&self) -> Option<&'a Box<ThreadContextData>> {
        let shared = ThreadPool::shared();
        shared.get(*self)
    }

    #[inline]
    #[track_caller]
    fn as_ref<'a>(&self) -> &'a ThreadContextData {
        self.get().unwrap()
    }

    #[inline]
    #[track_caller]
    unsafe fn unsafe_weak<'a>(&self) -> Option<&'a mut Box<ThreadContextData>> {
        let shared = ThreadPool::shared();
        shared.unsafe_weak(*self)
    }

    #[inline]
    pub fn name(&self) -> Option<&str> {
        self.get().and_then(|v| v.name())
    }

    #[inline]
    pub fn wake(&self) {
        self.as_ref().attribute.insert(ThreadAttributes::AWAKE);
        Scheduler::add(*self);
    }

    #[inline]
    pub fn join(&self) {
        self.get().map(|thread| thread.sem.wait());
    }

    fn update_statistics(&self) {
        self.update(|thread| {
            let now = Timer::measure().0;
            let then = thread.measure.swap(now, Ordering::SeqCst);
            let diff = now - then;
            thread.cpu_time.fetch_add(diff, Ordering::SeqCst);
            thread.load0.fetch_add(diff as u32, Ordering::SeqCst);
        });
    }
}

type ThreadStart = fn(usize) -> ();

#[allow(dead_code)]
struct ThreadContextData {
    /// Architectural context data
    context: CpuContextData,
    stack: Option<Box<[u8]>>,

    // IDs
    pid: ProcessId,
    handle: ThreadHandle,

    // Properties
    sem: Semaphore,
    personality: Option<Box<dyn Personality>>,
    attribute: AtomicBitflags<ThreadAttributes>,
    priority: Priority,
    quantum: Quantum,

    // Statistics
    measure: AtomicUsize,
    cpu_time: AtomicUsize,
    load0: AtomicU32,
    load: AtomicU32,

    // Executor
    executor: Option<Executor>,

    // Thread Name
    name: [u8; CONTEXT_LABEL_LENGTH],
}

bitflags! {
    struct ThreadAttributes: usize {
        const QUEUED    = 0b0000_0000_0000_0001;
        const ASLEEP    = 0b0000_0000_0000_0010;
        const AWAKE     = 0b0000_0000_0000_0100;
        const ZOMBIE    = 0b0000_0000_0000_1000;
    }
}

impl Into<usize> for ThreadAttributes {
    fn into(self) -> usize {
        self.bits()
    }
}

impl AtomicBitflags<ThreadAttributes> {
    fn to_char(&self) -> char {
        if self.contains(ThreadAttributes::ZOMBIE) {
            'z'
        } else if self.contains(ThreadAttributes::AWAKE) {
            'w'
        } else if self.contains(ThreadAttributes::ASLEEP) {
            'S'
        } else if self.contains(ThreadAttributes::QUEUED) {
            'R'
        } else {
            '-'
        }
    }
}

use core::fmt;
impl fmt::Display for AtomicBitflags<ThreadAttributes> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

#[allow(dead_code)]
impl ThreadContextData {
    fn new(
        pid: ProcessId,
        priority: Priority,
        name: &str,
        start: Option<ThreadStart>,
        arg: usize,
        personality: Option<Box<dyn Personality>>,
    ) -> ThreadHandle {
        let handle = ThreadHandle::next();

        let mut name_array = [0; CONTEXT_LABEL_LENGTH];
        set_name_array(&mut name_array, name);

        let mut thread = Self {
            context: CpuContextData::new(),
            stack: None,
            pid,
            handle,
            sem: Semaphore::new(0),
            attribute: AtomicBitflags::empty(),
            priority,
            quantum: Quantum::from(priority),
            measure: AtomicUsize::new(0),
            cpu_time: AtomicUsize::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            executor: None,
            personality,
            name: name_array,
        };
        if let Some(start) = start {
            unsafe {
                let size_of_stack = CpuContextData::SIZE_OF_STACK;
                let mut stack = Vec::with_capacity(size_of_stack);
                stack.resize(size_of_stack, 0);
                let stack = stack.into_boxed_slice();
                thread.stack = Some(stack);
                let stack = thread.stack.as_mut().unwrap().as_mut_ptr() as *mut c_void;
                thread
                    .context
                    .init(stack.add(size_of_stack), start as usize, arg);
            }
        }
        ThreadPool::add(Box::new(thread));
        handle
    }

    fn exit(&mut self) -> ! {
        Scheduler::yield_thread();

        self.sem.signal();
        self.personality.as_mut().map(|v| v.on_exit());
        self.personality = None;

        let process = self.pid.get().unwrap();
        if process.n_threads.fetch_sub(1, Ordering::SeqCst) == 1 {
            process.exit();
        }

        self.attribute.insert(ThreadAttributes::ZOMBIE);
        Scheduler::sleep();
        unreachable!();
    }

    fn set_name(&mut self, name: &str) {
        set_name_array(&mut self.name, name);
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

// impl Drop for RawThread {
//     fn drop(&mut self) {
//         println!("DROP THREAD {}", self.handle.0.get());
//     }
// }

// #[repr(transparent)]
// struct ThreadQueue(ArrayQueue<NonZeroUsize>);

// impl ThreadQueue {
//     #[inline]
//     fn with_capacity(capacity: usize) -> Self {
//         Self(ArrayQueue::new(capacity))
//     }

//     #[inline]
//     fn dequeue(&self) -> Option<ThreadHandle> {
//         self.0.pop().map(|v| ThreadHandle(v))
//     }

//     #[inline]
//     fn enqueue(&self, data: ThreadHandle) -> Result<(), ()> {
//         self.0.push(data.0).map_err(|_| ())
//     }
// }

struct ThreadQueue {
    lock: Spinlock,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    slice: UnsafeCell<Box<[usize]>>,
}

impl ThreadQueue {
    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        let cap = (capacity + 1).next_power_of_two();
        let mask = cap - 1;
        let mut vec = Vec::with_capacity(cap);
        vec.resize(cap, 0);
        Self {
            lock: Spinlock::new(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            mask,
            slice: UnsafeCell::new(vec.into_boxed_slice()),
        }
    }

    #[inline]
    fn dequeue(&self) -> Option<ThreadHandle> {
        unsafe {
            without_interrupts! {
                self.lock.synchronized(|| {
                    let mask = self.mask;
                    let head = mask & self.head.load(Ordering::Relaxed);
                    let tail = mask & self.tail.load(Ordering::Relaxed);
                    (head != tail)
                        .then(|| {
                            self.head.fetch_add(1, Ordering::SeqCst);
                            let slice = &*self.slice.get();
                            let a = slice.get_unchecked(head);
                            NonZeroUsize::new(*a).map(|v| ThreadHandle(v))
                        })
                        .flatten()
                })
            }
        }
    }

    #[inline]
    fn enqueue(&self, data: ThreadHandle) -> Result<(), ()> {
        unsafe {
            without_interrupts! {
                self.lock.synchronized(|| {
                    let mask = self.mask;
                    let head = mask & self.head.load(Ordering::Relaxed);
                    let tail = mask & self.tail.load(Ordering::Relaxed);
                    let new_tail = mask & (tail + 1);
                    (head != new_tail)
                        .then(|| {
                            self.tail.fetch_add(1, Ordering::SeqCst);
                            let slice = &mut *self.slice.get();
                            let a = slice.get_unchecked_mut(tail);
                            *a = data.as_usize();
                        })
                        .ok_or(())
                })
            }
        }
    }
}

/// Interrupt Request Level
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum Irql {
    Passive = 0,
    Apc,
    Dispatch,
    DIrql,
    IPI,
    High,
}

impl Irql {
    #[inline]
    pub fn current() -> Irql {
        unsafe {
            Scheduler::local_scheduler()
                .map(|v| v.current_irql())
                .unwrap_or(Irql::Passive)
        }
    }

    #[inline]
    #[track_caller]
    pub unsafe fn raise<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        without_interrupts!(match Scheduler::local_scheduler() {
            Some(lsch) => {
                let old_irql = lsch.raise_irql(*self);
                let r = f();
                Scheduler::local_scheduler().unwrap().lower_irql(old_irql);
                r
            }
            // TODO: can't change irql
            None => f(),
        })
    }
}

// #[derive(Debug)]
// #[allow(non_camel_case_types)]
// enum IrqlError {
//     IRQL_NOT_GREATER_OR_EQUAL(Irql, Irql),
//     IRQL_NOT_LESS_OR_EQUAL(Irql, Irql),
// }
