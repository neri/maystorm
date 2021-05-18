// Task Executor

use super::{Task, TaskId};
use crate::sync::semaphore::*;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::task::Waker;
use core::task::{Context, Poll};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<TaskQueue>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: TaskQueue::new(),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!();
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    fn run_ready_task(&mut self) {
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue,
            };
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
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
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
    queue: ArrayQueue<TaskId>,
    sem: Semaphore,
}

impl TaskQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: ArrayQueue::new(100),
            sem: Semaphore::new(0),
        })
    }

    fn push(&self, task_id: TaskId) -> Result<(), TaskId> {
        self.queue.push(task_id).map(|_| self.sem.signal())
    }

    fn pop(&self) -> Option<TaskId> {
        self.queue.pop()
    }

    fn wait(&self) {
        self.sem.wait();
    }
}
