// Error
// Most of them are clones of Rust's original definition.

use crate::error;
use alloc::boxed::Box;
use core::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    Other,
    UnexpectedEof,
}

pub struct Error {
    repr: Repr,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.repr, f)
    }
}

#[derive(Debug)]
pub enum Repr {
    Os(i32),
    Simple(ErrorKind),
    Custom(Box<Custom>),
}

#[derive(Debug)]
pub struct Custom {
    kind: ErrorKind,
    error: Box<dyn error::Error + Send + Sync>,
}

impl Error {
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn error::Error + Send + Sync>>,
    {
        Self {
            repr: Repr::Custom(Box::new(Custom {
                kind,
                error: error.into(),
            })),
        }
    }

    pub fn last_os_error() -> Self {
        todo!()
    }

    pub fn from_raw_os_error(code: i32) -> Self {
        Self {
            repr: Repr::Os(code),
        }
    }

    pub fn raw_os_error(&self) -> Option<i32> {
        if let Repr::Os(v) = self.repr {
            Some(v)
        } else {
            None
        }
    }

    pub fn get_ref(&self) -> Option<&(dyn error::Error + Send + Sync + 'static)> {
        if let Repr::Custom(ref v) = self.repr {
            Some(&*v.error)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut (dyn error::Error + Send + Sync + 'static)> {
        if let Repr::Custom(ref mut v) = self.repr {
            Some(&mut *v.error)
        } else {
            None
        }
    }

    pub fn into_inner(self) -> Option<Box<dyn error::Error + Send + Sync>> {
        if let Repr::Custom(v) = self.repr {
            Some(v.error)
        } else {
            None
        }
    }

    pub fn kind(&self) -> ErrorKind {
        match self.repr {
            Repr::Os(_) => todo!(),
            Repr::Simple(kind) => kind,
            Repr::Custom(ref v) => v.kind,
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(val: ErrorKind) -> Self {
        Self {
            repr: Repr::Simple(val),
        }
    }
}
