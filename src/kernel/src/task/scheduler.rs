// Thread Scheduler

use crate::arch::cpu::Cpu;
use crate::mem::memory::*;
use crate::mem::string::*;
use crate::sync::spinlock::*;
use crate::system::*;
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
// use bitflags::*;

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
    retired: ThreadQueue,
    locals: Vec<Box<LocalScheduler>>,
    pool: ThreadPool,
    usage: AtomicU32,
    is_frozen: AtomicBool,
}

impl MyScheduler {
    pub(crate) fn start(f: fn(usize) -> (), args: usize) -> ! {
        const SIZE_OF_URGENT_QUEUE: usize = 512;
        const SIZE_OF_MAIN_QUEUE: usize = 512;

        let urgent = ThreadQueue::with_capacity(SIZE_OF_URGENT_QUEUE);
        let ready = ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE);
        let retired = ThreadQueue::with_capacity(SIZE_OF_MAIN_QUEUE);
        let pool = ThreadPool::default();

        let locals = Vec::new();

        unsafe {
            SCHEDULER = Some(Box::new(Self {
                urgent,
                ready,
                retired,
                locals,
                pool,
                usage: AtomicU32::new(0),
                is_frozen: AtomicBool::new(false),
            }));
        }

        let sch = Self::shared();
        for index in 0..System::num_of_active_cpus() {
            sch.locals.push(LocalScheduler::new(ProcessorIndex(index)));
        }

        SpawnOption::new()
            .priority(Priority::Realtime)
            .new_pid()
            .spawn_f(Self::scheduler_thread, 0, "Scheduler");

        Self::spawn_f(f, args, "Kernel", SpawnOption::new().new_pid());

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

    pub fn current_pid() -> ProcessId {
        if Self::is_enabled() {
            Self::current_thread().as_ref().pid
        } else {
            ProcessId(0)
        }
    }

    pub fn current_thread() -> ThreadHandle {
        Self::local_scheduler().current_thread()
    }

    fn next_thread_id() -> ThreadId {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::SeqCst))
    }

    fn next_pid() -> ProcessId {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
        ProcessId(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }

    // Perform a Preemption
    pub(crate) fn reschedule() {
        if Self::is_enabled() {
            unsafe {
                Cpu::without_interrupts(|| {
                    let lsch = Self::local_scheduler();
                    if lsch.current.as_ref().priority != Priority::Realtime {
                        if lsch.current.update(|current| current.quantum.consume()) {
                            LocalScheduler::switch_context(lsch);
                        }
                    }
                });
            }
        }
    }

    /// Wait for Event or Timer
    pub fn wait_for(object: Option<&SignallingObject>, duration: Duration) {
        unsafe {
            Cpu::without_interrupts(|| {
                let lsch = Self::local_scheduler();
                let current = lsch.current;
                if let Some(object) = object {
                    let _ = object.set(current);
                }
                current.update(|current| {
                    current.deadline = Timer::new(duration);
                });
                LocalScheduler::switch_context(lsch);
            });
        }
    }

    pub fn signal(object: &SignallingObject) {
        if let Some(thread) = object.unbox() {
            thread.update(|thread| thread.deadline = Timer::JUST);
        }
    }

    #[inline]
    #[track_caller]
    fn local_scheduler() -> &'static mut LocalScheduler {
        let sch = Self::shared();
        let cpu_index = Cpu::current_processor_index().unwrap();
        sch.locals.get_mut(cpu_index.0).unwrap()
    }

    // Get Next Thread from queue
    fn next() -> Option<ThreadHandle> {
        let sch = Self::shared();
        if sch.is_frozen.load(Ordering::SeqCst) {
            return None;
        }
        for _ in 0..1 {
            if let Some(next) = sch.urgent.dequeue() {
                return Some(next);
            }
            while let Some(next) = sch.ready.dequeue() {
                if next.as_ref().deadline.until() {
                    MyScheduler::retire(next);
                    continue;
                } else {
                    return Some(next);
                }
            }
            let front = &sch.ready;
            let back = &sch.retired;
            while let Some(retired) = back.dequeue() {
                front.enqueue(retired).unwrap();
            }
        }
        None
    }

    // Retire Thread
    fn retire(thread: ThreadHandle) {
        let sch = Self::shared();
        let priority = thread.as_ref().priority;
        if priority != Priority::Idle {
            sch.retired.enqueue(thread).unwrap();
        }
    }

    fn scheduler_thread(_args: usize) {
        loop {
            Self::wait_for(None, Duration::from_secs(1));

            let sch = Self::shared();
            let mut usage = 0;
            for thread in sch.pool.dic.values() {
                let load = thread.load0.load(Ordering::SeqCst);
                thread.load.store(load, Ordering::SeqCst);
                thread.load0.fetch_sub(load, Ordering::SeqCst);
                if thread.priority != Priority::Idle {
                    usage += load;
                }
            }
            sch.usage.store(
                u32::min(usage / System::num_of_active_cpus() as u32, 1_000_000),
                Ordering::SeqCst,
            );
        }
    }

    pub fn usage() -> u32 {
        let sch = Self::shared();
        sch.usage.load(Ordering::Relaxed) / 1000
    }

    pub fn is_enabled() -> bool {
        unsafe { &SCHEDULER }.is_some() && SCHEDULER_ENABLED.load(Ordering::SeqCst)
    }

    pub(crate) unsafe fn freeze(force: bool) -> Result<(), ()> {
        let sch = Self::shared();
        sch.is_frozen.store(true, Ordering::SeqCst);
        if force {
            // TODO:
        }
        Ok(())
    }

    pub fn spawn_f(start: ThreadStart, args: usize, name: &str, options: SpawnOption) {
        assert!(options.priority.useful());
        let pid = if options.raise_pid {
            Self::next_pid()
        } else {
            Self::current_pid()
        };
        let thread = RawThread::new(pid, options.priority, name, Some(start), args);
        Self::retire(thread);
    }

    pub fn spawn<F>(_f: F)
    where
        F: FnOnce() -> (),
    {
        // assert!(priority.useful());
        todo!();
    }

    pub fn print_statistics(sb: &mut StringBuffer) {
        let sch = Self::shared();
        sb.clear();
        writeln!(sb, "PID THID Quan Pri Usage CPU Time Name").unwrap();
        for thread in sch.pool.dic.values() {
            let load = u32::min(thread.load.load(Ordering::Relaxed) / 1_000, 999);
            let load0 = load % 10;
            let load1 = load / 10;

            let time = thread.cpu_time.load(Ordering::Relaxed) / 10_000;
            let dsec = time % 100;
            let sec = time / 100 % 60;
            let min = time / 60_00 % 60;
            let hour = time / 3600_00;

            writeln!(
                sb,
                "{:3} {:3} {:2}/{:2} {:1} {:2}.{:1} {:3}:{:02}:{:02}.{:02} {}",
                thread.pid.0,
                thread.id.0,
                thread.quantum.current,
                thread.quantum.default,
                thread.priority as usize,
                load1,
                load0,
                hour,
                min,
                sec,
                dsec,
                thread.name().unwrap_or("")
            )
            .unwrap();
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
        let idle = RawThread::new(ProcessId(0), Priority::Idle, sb.as_str(), None, 0);
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
        let next = MyScheduler::next().unwrap_or(lsch.idle);
        current.update(|thread| {
            let now = Timer::monotonic().as_micros() as u64;
            let diff = now - thread.measure.load(Ordering::SeqCst);
            thread.cpu_time.fetch_add(diff, Ordering::SeqCst);
            thread.load0.fetch_add(diff as u32, Ordering::SeqCst);
            thread.measure.store(now, Ordering::SeqCst);
        });
        if current.as_ref().id != next.as_ref().id {
            lsch.retired = Some(current);
            lsch.current = next;

            //-//-//-//-//
            asm_sch_switch_context(
                &current.as_ref().context as *const _ as *mut _,
                &next.as_ref().context as *const _ as *mut _,
            );
            //-//-//-//-//

            let lsch = MyScheduler::local_scheduler();
            let current = lsch.current;
            current.update(|thread| {
                thread
                    .measure
                    .store(Timer::monotonic().as_micros() as u64, Ordering::SeqCst);
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
    let lsch = MyScheduler::local_scheduler();
    let current = lsch.current;
    current.update(|thread| {
        thread
            .measure
            .store(Timer::monotonic().as_micros() as u64, Ordering::SeqCst);
    });
    if let Some(retired) = lsch.retired {
        lsch.retired = None;
        MyScheduler::retire(retired);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SpawnOption {
    pub priority: Priority,
    pub raise_pid: bool,
}

impl SpawnOption {
    pub const fn new() -> Self {
        Self {
            priority: Priority::Normal,
            raise_pid: false,
        }
    }

    pub const fn with_priority(priority: Priority) -> Self {
        Self {
            priority,
            raise_pid: false,
        }
    }

    pub const fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub const fn new_pid(mut self) -> Self {
        self.raise_pid = true;
        self
    }

    pub fn spawn_f(self, start: fn(usize), args: usize, name: &str) {
        MyScheduler::spawn_f(start, args, name, self)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ProcessId(pub usize);

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ThreadId(pub usize);

static mut TIMER_SOURCE: Option<Box<dyn TimerSource>> = None;

pub type TimeSpec = u64;

pub trait TimerSource {
    fn create(&self, h: Duration) -> TimeSpec;
    #[must_use]
    fn until(&self, h: TimeSpec) -> bool;
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
    pub fn is_just(&self) -> bool {
        self.deadline == 0
    }

    #[must_use]
    pub fn until(self) -> bool {
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

    pub fn sleep(duration: Duration) {
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
        Self::sleep(Duration::from_micros(us));
    }

    #[inline]
    pub fn monotonic() -> Duration {
        unsafe { TIMER_SOURCE.as_ref() }.unwrap().monotonic()
    }
}

#[repr(u8)]
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
    dic: BTreeMap<ThreadHandle, Box<RawThread>>,
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

    fn add(thread: Box<RawThread>) -> ThreadHandle {
        let id = Self::synchronized(|| {
            let shared = Self::shared();
            let handle = ThreadHandle::new(thread.id.0).unwrap();
            shared.dic.insert(handle, thread);
            handle
        });
        id
    }

    fn get(&self, key: &ThreadHandle) -> Option<&Box<RawThread>> {
        Self::synchronized(|| self.dic.get(key))
    }

    fn get_mut(&mut self, key: &ThreadHandle) -> Option<&mut Box<RawThread>> {
        Self::synchronized(move || self.dic.get_mut(key))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ThreadHandle(NonZeroUsize);

impl ThreadHandle {
    #[inline]
    pub fn new(val: usize) -> Option<Self> {
        NonZeroUsize::new(val).map(|x| Self(x))
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0.get()
    }

    #[inline]
    fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RawThread) -> R,
    {
        let shared = ThreadPool::shared();
        let thread = shared.get_mut(self).unwrap();
        f(thread)
    }

    fn as_ref<'a>(&self) -> &'a RawThread {
        let shared = ThreadPool::shared();
        shared.get(self).as_ref().unwrap()
    }
}

const SIZE_OF_CONTEXT: usize = 512;
const SIZE_OF_STACK: usize = 0x10000;
const THREAD_NAME_LENGTH: usize = 32;

type ThreadStart = fn(usize) -> ();

#[allow(dead_code)]
struct RawThread {
    context: [u8; SIZE_OF_CONTEXT],
    pid: ProcessId,
    id: ThreadId,
    priority: Priority,
    quantum: Quantum,
    deadline: Timer,
    measure: AtomicU64,
    cpu_time: AtomicU64,
    load0: AtomicU32,
    load: AtomicU32,
    name: [u8; THREAD_NAME_LENGTH],
}

#[allow(dead_code)]
impl RawThread {
    fn new(
        pid: ProcessId,
        priority: Priority,
        name: &str,
        start: Option<ThreadStart>,
        args: usize,
    ) -> ThreadHandle {
        let mut thread = Self {
            context: [0; SIZE_OF_CONTEXT],
            pid,
            id: MyScheduler::next_thread_id(),
            priority,
            quantum: Quantum::from(priority),
            deadline: Timer::JUST,
            measure: AtomicU64::new(0),
            cpu_time: AtomicU64::new(0),
            load0: AtomicU32::new(0),
            load: AtomicU32::new(0),
            name: [0; THREAD_NAME_LENGTH],
        };
        thread.set_name(name);
        let handle = ThreadPool::add(Box::new(thread));
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

    fn exit(_exit_code: usize) -> ! {
        unimplemented!();
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
        array[0] = i as u8;
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
        Self(AtomicUsize::new(MyScheduler::current_thread().as_usize()))
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
        ThreadHandle::new(self.0.load(Ordering::Acquire))
    }

    pub fn unbox(&self) -> Option<ThreadHandle> {
        ThreadHandle::new(self.0.swap(Self::NONE, Ordering::AcqRel))
    }

    pub fn wait(&self, duration: Duration) {
        MyScheduler::wait_for(Some(self), duration)
    }

    pub fn signal(&self) {
        MyScheduler::signal(self)
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
