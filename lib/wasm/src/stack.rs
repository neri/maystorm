// Stack structures of Wasm runtime

use crate::*;
use alloc::vec::Vec;
use core::{cell::UnsafeCell, mem::align_of, mem::size_of, slice};

#[derive(Copy, Clone)]
pub union WasmStackValue {
    i32: i32,
    u32: u32,
    i64: i64,
    u64: u64,
    f32: f32,
    f64: f64,
    usize: usize,
    isize: isize,
}

impl WasmStackValue {
    #[inline]
    pub const fn zero() -> Self {
        Self { u64: 0 }
    }

    #[inline]
    pub const fn from_bool(v: bool) -> Self {
        if v {
            Self::from_usize(1)
        } else {
            Self::from_usize(0)
        }
    }

    #[inline]
    pub const fn from_usize(v: usize) -> Self {
        Self { usize: v }
    }

    #[inline]
    pub const fn from_isize(v: isize) -> Self {
        Self { isize: v }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        Self { i32: v }
    }

    #[inline]
    pub const fn from_u32(v: u32) -> Self {
        Self { u32: v }
    }

    #[inline]
    pub const fn from_i64(v: i64) -> Self {
        Self { i64: v }
    }

    #[inline]
    pub const fn from_u64(v: u64) -> Self {
        Self { u64: v }
    }

    #[inline]
    pub fn get_bool(&self) -> bool {
        unsafe { self.i32 != 0 }
    }

    #[inline]
    pub fn get_i32(&self) -> i32 {
        unsafe { self.i32 }
    }

    #[inline]
    pub fn get_u32(&self) -> u32 {
        unsafe { self.u32 }
    }

    #[inline]
    pub fn get_i64(&self) -> i64 {
        unsafe { self.i64 }
    }

    #[inline]
    pub fn get_u64(&self) -> u64 {
        unsafe { self.u64 }
    }

    #[inline]
    pub fn get_f32(&self) -> f32 {
        unsafe { self.f32 }
    }

    #[inline]
    pub fn get_f64(&self) -> f64 {
        unsafe { self.f64 }
    }

    #[inline]
    pub fn get_i8(&self) -> i8 {
        unsafe { self.usize as i8 }
    }

    #[inline]
    pub fn get_u8(&self) -> u8 {
        unsafe { self.usize as u8 }
    }

    #[inline]
    pub fn get_i16(&self) -> i16 {
        unsafe { self.usize as i16 }
    }

    #[inline]
    pub fn get_u16(&self) -> u16 {
        unsafe { self.usize as u16 }
    }

    #[inline]
    pub fn map_i32<F>(&mut self, f: F)
    where
        F: FnOnce(i32) -> i32,
    {
        let val = unsafe { self.i32 };
        self.i32 = f(val);
    }

    #[inline]
    pub fn map_u32<F>(&mut self, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        let val = unsafe { self.u32 };
        self.u32 = f(val);
    }

    #[inline]
    pub fn map_i64<F>(&mut self, f: F)
    where
        F: FnOnce(i64) -> i64,
    {
        let val = unsafe { self.i64 };
        self.i64 = f(val);
    }

    #[inline]
    pub fn map_u64<F>(&mut self, f: F)
    where
        F: FnOnce(u64) -> u64,
    {
        let val = unsafe { self.u64 };
        self.u64 = f(val);
    }

    #[inline]
    pub fn map_isize<F>(&mut self, f: F)
    where
        F: FnOnce(isize) -> isize,
    {
        let val = unsafe { self.isize };
        self.isize = f(val);
    }

    #[inline]
    pub fn map_usize<F>(&mut self, f: F)
    where
        F: FnOnce(usize) -> usize,
    {
        let val = unsafe { self.usize };
        self.usize = f(val);
    }

    #[inline]
    pub fn get_by_type(&self, val_type: WasmValType) -> WasmValue {
        match val_type {
            WasmValType::I32 => WasmValue::I32(self.get_i32()),
            WasmValType::I64 => WasmValue::I64(self.get_i64()),
            // WasmValType::F32 => {}
            // WasmValType::F64 => {}
            _ => todo!(),
        }
    }

    #[inline]
    pub fn into_value(&self, val_type: WasmValType) -> WasmValue {
        match val_type {
            WasmValType::I32 => WasmValue::I32(self.get_i32()),
            WasmValType::I64 => WasmValue::I64(self.get_i64()),
            // WasmValType::F32 => {}
            // WasmValType::F64 => {}
            _ => todo!(),
        }
    }
}

impl From<bool> for WasmStackValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::from_bool(v)
    }
}

impl From<usize> for WasmStackValue {
    #[inline]
    fn from(v: usize) -> Self {
        Self::from_usize(v)
    }
}

impl From<u32> for WasmStackValue {
    #[inline]
    fn from(v: u32) -> Self {
        Self::from_u32(v)
    }
}

impl From<i32> for WasmStackValue {
    #[inline]
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<u64> for WasmStackValue {
    #[inline]
    fn from(v: u64) -> Self {
        Self::from_u64(v)
    }
}

impl From<i64> for WasmStackValue {
    #[inline]
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl From<WasmValue> for WasmStackValue {
    #[inline]
    fn from(v: WasmValue) -> Self {
        match v {
            WasmValue::Empty => Self::from_i64(0),
            WasmValue::I32(v) => Self::from_i64(v as i64),
            WasmValue::I64(v) => Self::from_i64(v),
            _ => todo!(),
        }
    }
}

/// Fixed Size Stack
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
pub struct SharedStack {
    vec: UnsafeCell<Vec<u8>>,
    stack_pointer: usize,
}

impl SharedStack {
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
    use super::{FixedStack, SharedStack};

    #[test]
    fn stack() {
        let mut pool = SharedStack::new();

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
