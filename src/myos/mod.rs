pub mod arch;
pub mod bus;
pub mod io;
pub mod mem;
pub mod mux;
pub mod num;
pub mod thread;
pub mod scheduler;

static VERSION: Version = Version::new(0, 0, 1);

pub struct MyOs {}

impl MyOs {
    pub fn version() -> Version {
        VERSION
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub maj: usize,
    pub min: usize,
    pub rev: usize,
}

impl Version {
    pub const fn new(maj: usize, min: usize, rev: usize) -> Self {
        Version {
            maj: maj,
            min: min,
            rev: rev,
        }
    }
}

use core::fmt;
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.maj, self.min, self.rev)
    }
}
