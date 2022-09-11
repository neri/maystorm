use core::{fmt, slice, str};

/// Small String Buffer
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Ord)]
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
    pub fn backspace(&mut self) {
        let len = self.len();
        if len > 0 {
            self.0[0] -= 1;
        }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.0[0] as usize
    }

    #[inline]
    fn as_slice<'a>(&'a self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.0.as_ptr().add(1), self.len()) }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        str::from_utf8(self.as_slice()).unwrap_or("")
    }

    #[inline]
    pub unsafe fn as_str_unchecked(&self) -> &str {
        str::from_utf8_unchecked(self.as_slice())
    }
}

impl PartialEq for Sb255 {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl PartialOrd for Sb255 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
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

impl AsRef<str> for Sb255 {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
