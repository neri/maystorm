// Atomic Linked Queue - WIP

use crate::myos::arch::cpu::Cpu;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::marker::PhantomData;
use core::num::*;
use core::sync::atomic::*;

#[derive(Copy, Clone, PartialOrd, PartialEq)]
struct LinkedListIndex(pub usize);

impl LinkedListIndex {
    const NULL: LinkedListIndex = LinkedListIndex(0);
}

struct AtomicLinkedNode {
    link: AtomicLinkedListIndex,
    value: AtomicUsize,
}

impl AtomicLinkedNode {
    const fn new(link: AtomicLinkedListIndex) -> Self {
        Self {
            link: link,
            value: AtomicUsize::new(0),
        }
    }
}

struct AtomicLinkedListIndex(AtomicUsize);

impl AtomicLinkedListIndex {
    const fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }

    fn load(&self) -> LinkedListIndex {
        LinkedListIndex(self.0.load(Ordering::Relaxed))
    }

    fn store(&self, value: LinkedListIndex) {
        self.0.store(value.0, Ordering::SeqCst);
    }

    fn cas(
        &self,
        expected: LinkedListIndex,
        desired: LinkedListIndex,
    ) -> Result<(), LinkedListIndex> {
        match self
            .0
            .compare_exchange(expected.0, desired.0, Ordering::SeqCst, Ordering::Relaxed)
        {
            Ok(_) => Ok(()),
            Err(v) => Err(LinkedListIndex(v)),
        }
    }
}

pub struct AtomicLinkedQueue<T> {
    head: AtomicLinkedListIndex,
    tail: AtomicLinkedListIndex,
    free: AtomicLinkedListIndex,
    pool: Vec<AtomicLinkedNode>,
    phantom: PhantomData<T>,
}

unsafe impl<T> Sync for AtomicLinkedQueue<T> {}

unsafe impl<T> Send for AtomicLinkedQueue<T> {}

impl<T> AtomicLinkedQueue<T>
where
    T: Into<NonZeroUsize> + From<NonZeroUsize>,
{
    pub fn with_capacity(capacity: usize) -> Box<Self> {
        let mut list = Box::new(Self {
            head: AtomicLinkedListIndex::new(1),
            tail: AtomicLinkedListIndex::new(1),
            free: AtomicLinkedListIndex::new(2),
            pool: Vec::with_capacity(capacity),
            phantom: PhantomData,
        });
        list.pool
            .push(AtomicLinkedNode::new(AtomicLinkedListIndex::new(0)));
        for i in 2..capacity {
            list.pool
                .push(AtomicLinkedNode::new(AtomicLinkedListIndex::new(i + 1)));
        }
        list.pool
            .push(AtomicLinkedNode::new(AtomicLinkedListIndex::new(0)));

        list
    }

    fn item_at(&self, index: LinkedListIndex) -> &AtomicLinkedNode {
        &self.pool[index.0 - 1]
    }

    pub fn dequeue_raw(&self) -> usize {
        loop {
            let head = self.head.load();
            let dummy = self.item_at(head);
            let target = dummy.link.load();
            if target != LinkedListIndex::NULL {
                let item = self.item_at(target);
                let result = item.value.load(Ordering::Relaxed);
                if self.head.cas(head, target).is_ok() {
                    loop {
                        let free = self.free.load();
                        dummy.link.store(free);
                        if self.free.cas(free, head).is_ok() {
                            break;
                        }
                        Cpu::relax();
                    }
                    // self.print_self("READ");
                    return result;
                }
            } else {
                return 0;
            }
            Cpu::relax();
        }
    }

    pub fn enqueue_raw(&self, value: usize) -> Result<(), ()> {
        let index = loop {
            let free = self.free.load();
            if free != LinkedListIndex::NULL {
                let item = self.item_at(free);
                if self.free.cas(free, item.link.load()).is_ok() {
                    item.value.store(value, Ordering::SeqCst);
                    item.link.store(LinkedListIndex::NULL);
                    // println!("W{} {:2x}", free.0, value);
                    break free;
                }
            } else {
                return Err(());
            }
            Cpu::relax();
        };
        loop {
            let tail = self.tail.load();
            let mut last_item = self.item_at(tail);
            while last_item.link.load() != LinkedListIndex::NULL {
                last_item = self.item_at(last_item.link.load());
            }
            if last_item.link.cas(LinkedListIndex::NULL, index).is_ok() {
                let _ = self.tail.cas(tail, index);
                // self.print_self("WRITE");
                return Ok(());
            }
            Cpu::relax();
        }
    }

    pub fn enqueue(&self, value: T) -> Result<(), ()> {
        self.enqueue_raw(value.into().get())
    }

    pub fn dequeue(&self) -> Option<T> {
        NonZeroUsize::new(self.dequeue_raw()).map(|x| x.into())
    }

    // fn print_self(&self, s: &str) {
    //     println!(
    //         "{} {:#?} h {} t {} f {}",
    //         s,
    //         self as *const _,
    //         self.head.load().0,
    //         self.tail.load().0,
    //         self.free.load().0,
    //     );
    // }
}
