//! Stack structure for the Webassembly Runtime

use alloc::vec::Vec;
use core::{cell::UnsafeCell, mem::align_of, mem::size_of, slice};

/// Fixed size stack
pub struct FixedStack<'a, T> {
    slice: &'a mut [T],
    stack_pointer: usize,
}

impl<'a, T> FixedStack<'a, T> {
    #[inline]
    pub fn from_slice(slice: &'a mut [T]) -> Self {
        Self {
            slice,
            stack_pointer: 0,
        }
    }
}

impl<T> FixedStack<'_, T> {
    #[inline]
    pub const fn len(&self) -> usize {
        self.stack_pointer
    }
}

impl<T: Sized + Copy + Clone> FixedStack<'_, T> {
    #[inline]
    pub fn remove_all(&mut self) {
        self.stack_pointer = 0;
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.slice[..self.stack_pointer]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.slice[..self.stack_pointer]
    }

    #[inline]
    pub fn last(&self) -> Option<&T> {
        if self.stack_pointer > 0 {
            self.slice.get(self.stack_pointer - 1)
        } else {
            None
        }
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.stack_pointer > 0 {
            self.slice.get_mut(self.stack_pointer - 1)
        } else {
            None
        }
    }

    #[inline]
    pub fn push(&mut self, data: T) -> Result<(), ()> {
        if self.stack_pointer < self.slice.len() {
            self.slice
                .get_mut(self.stack_pointer)
                .map(|v| *v = data)
                .map(|_| self.stack_pointer += 1)
                .ok_or(())
        } else {
            Err(())
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.stack_pointer > 0 {
            let new_sp = self.stack_pointer - 1;
            self.slice.get(new_sp).map(|v| *v).map(|v| {
                self.stack_pointer = new_sp;
                v
            })
        } else {
            None
        }
    }

    #[track_caller]
    pub fn resize(&mut self, new_size: usize, new_value: T) {
        if new_size < self.slice.len() {
            if self.stack_pointer < new_size {
                let _ = new_value;
                todo!();
            }
            self.stack_pointer = new_size;
        } else {
            self.slice[usize::MAX];
        }
    }

    #[track_caller]
    pub fn extend_from_slice(&mut self, other: &[T]) {
        if other.len() == 0 {
            return;
        }
        let count = other.len();
        let cur_size = self.stack_pointer;
        let new_size = cur_size + count;
        if new_size > self.slice.len() {
            self.slice[usize::MAX];
        }
        unsafe {
            let p = self.slice.get_unchecked_mut(cur_size) as *mut T;
            let q = other.get_unchecked(0) as *const T;
            q.copy_to_nonoverlapping(p, count);
        }
        self.stack_pointer = new_size;
    }
}

/// Shared Stack
pub struct StackHeap {
    vec: UnsafeCell<Vec<u8>>,
    stack_pointer: usize,
}

impl StackHeap {
    #[inline]
    pub const fn new() -> Self {
        Self {
            vec: UnsafeCell::new(Vec::new()),
            stack_pointer: 0,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: UnsafeCell::new(Vec::with_capacity(capacity)),
            stack_pointer: 0,
        }
    }

    #[inline]
    pub fn snapshot<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let Self { vec, stack_pointer } = self;

        let vec = unsafe {
            let dummy = Vec::new();
            vec.get().replace(dummy)
        };

        let mut child = Self {
            vec: UnsafeCell::new(vec),
            stack_pointer: *stack_pointer,
        };
        let r = f(&mut child);

        unsafe {
            self.vec.get().replace(child.vec.into_inner());
        }

        r
    }

    pub fn alloc<'a, T>(&mut self, len: usize) -> &'a mut [T]
    where
        T: Sized + Copy + Clone,
    {
        const MIN_PADDING: usize = 16;
        let align = usize::max(MIN_PADDING, align_of::<T>());
        let offset = (self.stack_pointer + align - 1) & !(align - 1);
        let vec_size = size_of::<T>() * len;
        let new_size = (offset + vec_size + MIN_PADDING - 1) & !(MIN_PADDING - 1);

        if self.vec.get_mut().len() < new_size {
            self.vec.get_mut().resize(new_size, 0);
        }

        let slice = unsafe {
            let base = self.vec.get_mut().as_mut_ptr().add(offset) as *const _ as *mut T;
            slice::from_raw_parts_mut(base, len)
        };

        self.stack_pointer = new_size;

        slice
    }

    #[inline]
    pub fn alloc_stack<'a, T>(&mut self, len: usize) -> FixedStack<'a, T>
    where
        T: Sized + Copy + Clone,
    {
        let slice = self.alloc(len);
        FixedStack::from_slice(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedStack, StackHeap};

    #[test]
    fn stack() {
        let mut pool = StackHeap::new();

        pool.snapshot(|stack| {
            assert_eq!(stack.stack_pointer, 0);
            let mut stack1: FixedStack<i32> = stack.alloc_stack(123);
            assert_eq!(stack.stack_pointer, 496);

            assert_eq!(stack1.stack_pointer, 0);
            assert_eq!(stack1.pop(), None);

            stack1.push(123).unwrap();
            assert_eq!(stack1.stack_pointer, 1);
        });
        assert_eq!(pool.stack_pointer, 0);
    }
}
