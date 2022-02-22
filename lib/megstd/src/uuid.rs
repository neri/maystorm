// Universally Unique Identifier

use core::{fmt::*, mem::transmute};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Uuid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

impl Uuid {
    #[inline]
    pub const fn from_parts(a: u32, b: u16, c: u16, d: u16, e: [u8; 6]) -> Uuid {
        Uuid {
            a,
            b,
            c,
            d: [
                (d / 0x100) as u8,
                (d % 0x100) as u8,
                e[0],
                e[1],
                e[2],
                e[3],
                e[4],
                e[5],
            ],
        }
    }

    pub const NULL: Self = Self::null();

    #[inline]
    pub const fn null() -> Uuid {
        Uuid {
            a: 0,
            b: 0,
            c: 0,
            d: [0; 8],
        }
    }

    #[inline]
    pub fn from_slice(slice: &[u8; 16]) -> &Self {
        unsafe { transmute(slice) }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8; 16] {
        unsafe { transmute(self) }
    }

    #[inline]
    pub fn version(&self) -> Option<UuidVersion> {
        FromPrimitive::from_u16(self.c >> 12)
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let d = ((self.d[0] as u16) << 8) + (self.d[1] as u16);

        let e = self.d[2..8]
            .iter()
            .fold(0, |acc, v| (acc << 8) + (*v as u64));

        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.a, self.b, self.c, d, e,
        )
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UuidVersion {
    V1 = 1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
}
