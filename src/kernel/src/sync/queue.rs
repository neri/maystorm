// Lock-free Queue - WIP

use crate::arch::cpu::Cpu;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::marker::PhantomData;
use core::num::*;
use core::sync::atomic::*;

pub struct AtomicLinkedQueue<T> {
    head: AtomicIndex,
    tail: AtomicIndex,
    free: AtomicIndex,
    pool: Vec<Node>,
    phantom: PhantomData<T>,
}

unsafe impl<T> Sync for AtomicLinkedQueue<T> {}

unsafe impl<T> Send for AtomicLinkedQueue<T> {}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
struct Index(pub usize);

impl Index {
    const NULL: Index = Index(0);
}

struct Node {
    link: AtomicIndex,
    value: AtomicUsize,
}

impl Node {
    const fn new(link: Index) -> Self {
        Self {
            link: AtomicIndex::new(link),
            value: AtomicUsize::new(0),
        }
    }
}

struct AtomicIndex(AtomicUsize);

impl AtomicIndex {
    const fn new(value: Index) -> Self {
        Self(AtomicUsize::new(value.0))
    }

    fn load(&self) -> Index {
        Index(self.0.load(Ordering::Acquire))
    }

    fn store(&self, value: Index) {
        self.0.store(value.0, Ordering::Release);
    }

    fn cas(&self, expected: Index, desired: Index) -> Result<(), Index> {
        match self
            .0
            .compare_exchange(expected.0, desired.0, Ordering::SeqCst, Ordering::Relaxed)
        {
            Ok(_) => Ok(()),
            Err(v) => Err(Index(v)),
        }
    }
}

impl<T> AtomicLinkedQueue<T>
where
    T: Into<NonZeroUsize> + From<NonZeroUsize>,
{
    pub fn with_capacity(capacity: usize) -> Box<Self> {
        let mut queue = Box::new(Self {
            head: AtomicIndex::new(Index(1)),
            tail: AtomicIndex::new(Index(1)),
            free: AtomicIndex::new(Index(2)),
            pool: Vec::with_capacity(capacity),
            phantom: PhantomData,
        });

        queue.pool.push(Node::new(Index::NULL));
        for i in 2..capacity {
            queue.pool.push(Node::new(Index(i + 1)));
        }
        queue.pool.push(Node::new(Index::NULL));

        queue
    }

    pub fn enqueue(&self, value: T) -> Result<(), ()> {
        self.enqueue_raw(value.into().get())
    }

    pub fn dequeue(&self) -> Option<T> {
        NonZeroUsize::new(self.dequeue_raw()).map(|v| v.into())
    }

    fn item_at(&self, index: Index) -> &Node {
        &self.pool[index.0 - 1]
    }

    pub fn dequeue_raw(&self) -> usize {
        loop {
            let head = self.head.load();
            let tail = self.tail.load();
            if head == tail {
                let last_item = self.item_at(tail);
                let next = last_item.link.load();
                if next == Index::NULL {
                    return 0;
                } else {
                    let _ = self.tail.cas(tail, next);
                    continue;
                }
            }
            let dummy = self.item_at(head);
            let target = dummy.link.load();
            if target != Index::NULL {
                let item = self.item_at(target);
                let result = item.value.load(Ordering::Acquire);
                if self.head.cas(head, target).is_ok() {
                    loop {
                        let free = self.free.load();
                        dummy.link.store(free);
                        if self.free.cas(free, head).is_ok() {
                            break;
                        }
                        Cpu::spin_loop_hint();
                    }
                    return result;
                }
            } else {
                return 0;
            }
            Cpu::spin_loop_hint();
        }
    }

    pub fn enqueue_raw(&self, value: usize) -> Result<(), ()> {
        let index = loop {
            let free = self.free.load();
            if free != Index::NULL {
                let item = self.item_at(free);
                if self.free.cas(free, item.link.load()).is_ok() {
                    item.value.store(value, Ordering::Release);
                    item.link.store(Index::NULL);
                    break free;
                }
            } else {
                return Err(());
            }
            Cpu::spin_loop_hint();
        };
        loop {
            let tail = self.tail.load();
            let mut last_item = self.item_at(tail);
            while last_item.link.load() != Index::NULL {
                last_item = self.item_at(last_item.link.load());
            }
            if last_item.link.cas(Index::NULL, index).is_ok() {
                let _ = self.tail.cas(tail, index);
                return Ok(());
            }
            Cpu::spin_loop_hint();
        }
    }
}
