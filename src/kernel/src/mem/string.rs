// Small String Buffer & Formatter

use core::{fmt, slice, str};

#[macro_export]
macro_rules! sformat {
    ($str:expr, $($arg:tt)*) => {
        $str.clear();
        write!($str, $($arg)*).unwrap();
    };
}

pub struct Str255([u8; 256]);

impl Str255 {
    pub const fn new() -> Self {
        Self([0; 256])
    }

    pub fn clear(&mut self) {
        self.0[0] = 0;
    }

    pub fn len(&self) -> usize {
        self.0[0] as usize
    }

    pub fn as_str<'a>(&self) -> &'a str {
        let len = self.len();
        let slice = unsafe { slice::from_raw_parts(&self.0[1], len) };
        unsafe { str::from_utf8_unchecked(slice) }
    }
}

impl fmt::Write for Str255 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut iter = 1 + self.len();
        self.0[0] += s.bytes().count() as u8;
        for c in s.bytes() {
            self.0[iter] = c;
            iter += 1;
        }
        Ok(())
    }
}
