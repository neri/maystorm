#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(async_closure)]
#![feature(box_into_inner)]
#![feature(cfg_target_has_atomic)]
#![feature(const_convert)]
#![feature(const_maybe_uninit_zeroed)]
#![feature(const_mut_refs)]
#![feature(const_option_ext)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(control_flow_enum)]
#![feature(core_intrinsics)]
#![feature(default_free_fn)]
#![feature(iter_advance_by)]
#![feature(lang_items)]
#![feature(maybe_uninit_uninit_array)]
#![feature(more_qualified_paths)]
#![feature(naked_functions)]
#![feature(negative_impls)]
#![feature(new_uninit)]
#![feature(option_result_contains)]
#![feature(panic_info_message)]
#![feature(trait_alias)]
#![feature(let_chains)]
#![feature(array_chunks)]
#![feature(step_trait)]
//-//-//-//
#![allow(incomplete_features)]
#![feature(return_position_impl_trait_in_trait)]

#[macro_use]
pub mod arch;

#[macro_use]
pub mod hal;

pub mod drivers;
pub mod fs;
pub mod fw;
pub mod io;
pub mod log;
pub mod mem;
pub mod num;
pub mod r;
pub mod res;
pub mod rt;
pub mod sync;
pub mod system;
pub mod task;
pub mod ui;
pub mod user;

pub use crate::hal::*;

use crate::system::System;
use bootprot::*;
use core::{fmt::Write, panic::PanicInfo};
use megstd::Box;

extern crate alloc;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        write!(system::System::stdout(), $($arg)*).unwrap()
    };
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => {
        print!(concat!($fmt, "\r\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*)
    };
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let _ = writeln!(log::Log::new(), $($arg)*).unwrap();
    };
}

static PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        Hal::cpu().disable_interrupt();
        task::scheduler::Scheduler::freeze(true);
        PANIC_GLOBAL_LOCK.synchronized(|| {
            let stdout = System::log();
            stdout.set_attribute(0x4F);
            let _ = writeln!(stdout, " = Guru Meditation = ");
            if let Some(thread) = task::scheduler::Scheduler::current_thread() {
                if let Some(name) = thread.name() {
                    let _ = write!(stdout, "thread '{}' ", name);
                } else {
                    let _ = write!(stdout, "thread {} ", thread.as_usize());
                }
            }
            let _ = writeln!(stdout, "{}", info);
        });
        Hal::cpu().stop();
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[repr(transparent)]
pub struct HexDump<'a>(pub &'a [u8]);

impl core::fmt::Debug for HexDump<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for c in self.0.iter() {
            write!(f, " {:02x}", *c)?;
        }
        Ok(())
    }
}

// like bitflags
#[macro_export]
macro_rules! my_bitflags {
    (
        $(#[$outer:meta])*
        $vis:vis struct $class:ident: $ty:ty {
            $(
                $(#[$attr:ident $($args:tt)*])*
                const $flag:ident = $value:expr;
            )*
        }
    ) => {

        $(#[$outer])*
        #[repr(transparent)]
        #[derive(Copy, Clone, Eq, PartialEq)]
        $vis struct $class($ty);

        impl $class {
            const __EMPTY: Self = Self(0);

            $(
                $(#[$attr $($args)*])*
                $vis const $flag: Self = Self($value);
            )*

            const __ALL: Self = Self(0
                $(| $value)*
            );
        }

        #[allow(dead_code)]
        impl $class {

            #[inline]
            pub const fn from_bits_retain(bits: $ty) -> Self {
                Self(bits)
            }

            #[inline]
            pub const fn from_bits_truncate(bits: $ty) -> Self {
                Self(bits & Self::__ALL.0)
            }

            #[inline]
            pub const fn bits(&self) -> $ty {
                self.0
            }

            #[inline]
            pub const fn empty() -> Self {
                Self::__EMPTY
            }

            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.bits() == Self::__EMPTY.bits()
            }

            #[inline]
            pub const fn all() -> Self {
                Self::__ALL
            }

            #[inline]
            pub const fn is_all(&self) -> bool {
                (self.bits() & Self::__ALL.bits()) == Self::__ALL.bits()
            }

            #[inline]
            pub const fn contains(&self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            #[inline]
            pub const fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }

            #[inline]
            pub const fn remove(&mut self, other: Self) {
                self.0 &= !other.0;
            }

            #[inline]
            pub const fn toggle(&mut self, other: Self) {
                self.0 ^= other.0;
            }

            #[inline]
            pub const fn set(&mut self, other: Self, value: bool) {
                if value {
                    self.insert(other);
                } else {
                    self.remove(other);
                }
            }

            #[inline]
            pub const fn intersects(&self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            #[inline]
            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }

            #[inline]
            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            #[inline]
            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }

            #[inline]
            #[must_use]
            pub const fn symmetric_difference(self, other: Self) -> Self {
                Self(self.0 ^ other.0)
            }

            #[inline]
            #[must_use]
            pub const fn complement(self) -> Self {
                Self(!self.0 & Self::__ALL.0)
            }
        }

        impl const core::ops::Not for $class {
            type Output = Self;

            #[inline]
            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl const core::ops::BitAnd<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.bits() & rhs.bits())
            }
        }

        impl const core::ops::BitAndAssign<Self> for $class {
            #[inline]
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl const core::ops::BitOr<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.bits() | rhs.bits())
            }
        }

        impl const core::ops::BitOrAssign<Self> for $class {
            #[inline]
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl const core::ops::BitXor<Self> for $class {
            type Output = Self;

            #[inline]
            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.bits() ^ rhs.bits())
            }
        }

        impl const core::ops::BitXorAssign<Self> for $class {
            #[inline]
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }

        impl const core::ops::Sub<Self> for $class {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 & !rhs.0)
            }
        }

        impl const core::ops::SubAssign<Self> for $class {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                self.0 &= !rhs.0;
            }
        }

        impl const From<$ty> for $class {
            #[inline]
            fn from(val: $ty) -> $class {
                $class::from_bits_retain(val)
            }
        }

        impl const From<$class> for $ty {
            #[inline]
            fn from(val: $class) -> $ty {
                val.0
            }
        }

    };
}
