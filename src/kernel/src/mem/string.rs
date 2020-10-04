// Small String Buffer & Formatter

use alloc::vec::Vec;
use core::{fmt, slice, str};

#[macro_export]
macro_rules! sformat {
    ($sb:expr, $($arg:tt)*) => {
        $sb.clear();
        write!($sb, $($arg)*).unwrap();
    };
}

pub struct Sb255([u8; 256]);

impl Sb255 {
    #[inline]
    pub const fn new() -> Self {
        Self([0; 256])
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0[0] = 0;
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.0[0] as usize
    }

    #[inline]
    pub fn as_str<'a>(&self) -> &'a str {
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(&self.0[1], self.len())) }
    }
}

impl fmt::Write for Sb255 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut iter = 1 + self.len();
        for c in s.bytes() {
            self.0[iter] = c;
            iter += 1;
        }
        self.0[0] += s.bytes().count() as u8;
        Ok(())
    }
}

pub struct StringBuffer(Vec<u8>);

impl StringBuffer {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    #[inline]
    pub fn clear(&mut self) {
        unsafe { self.0.set_len(0) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn as_str<'a>(&self) -> &'a str {
        match self.len() {
            0 => "",
            len => unsafe { str::from_utf8_unchecked(slice::from_raw_parts(&self.0[0], len)) },
        }
    }
}

impl fmt::Write for StringBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            self.0.push(c);
        }
        Ok(())
    }
}
