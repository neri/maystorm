// Task Executor

use super::{Task, TaskId};
use crate::{
    sync::fifo::*,
    sync::{semaphore::*, RwLock},
};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};

pub struct Executor {
    tasks: RwLock<BTreeMap<TaskId, Task>>,
    task_queue: Arc<TaskQueue>,
    waker_cache: RwLock<BTreeMap<TaskId, Waker>>,
    spawn_queue: ConcurrentFifo<Task>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: RwLock::new(BTreeMap::new()),
            task_queue: TaskQueue::new(),
            waker_cache: RwLock::new(BTreeMap::new()),
            spawn_queue: ConcurrentFifo::with_capacity(100),
        }
    }

    pub fn spawn(&self, task: Task) {
        let _ = self.spawn_queue.enqueue(task);
    }

    fn spawn_internal(&self, task: Task) {
        let task_id = task.id;
        if self.tasks.write().unwrap().insert(task.id, task).is_some() {
            panic!();
        }
        self.task_queue.push(task_id).expect("task queue full");
    }

    fn run_ready_task(&self) {
        let Self {
            tasks,
            task_queue,
            waker_cache,
            spawn_queue: _,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let mut tasks = tasks.write().unwrap();
            let mut waker_cache = waker_cache.write().unwrap();

            let Some(task) = tasks.get_mut(&task_id) else { continue };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }

            drop(tasks);
            drop(waker_cache);
        }
    }

    pub fn run(&self) -> ! {
        loop {
            self.run_ready_task();
            while let Some(task) = self.spawn_queue.dequeue() {
                self.spawn_internal(task);
            }
            self.run_ready_task();
            self.task_queue.wait();
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<TaskQueue>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<TaskQueue>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full")
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

struct TaskQueue {
    queue: ConcurrentFifo<TaskId>,
    sem: Semaphore,
}

impl TaskQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: ConcurrentFifo::with_capacity(100),
            sem: Semaphore::new(0),
        })
    }

    fn push(&self, task_id: TaskId) -> Result<(), TaskId> {
        self.queue.enqueue(task_id).map(|_| self.sem.signal())
    }

    fn pop(&self) -> Option<TaskId> {
        self.queue.dequeue()
    }

    fn wait(&self) {
        self.sem.wait();
    }
}
