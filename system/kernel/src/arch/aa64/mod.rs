mod hal_aa64;
pub use hal_aa64::*;

use crate::{assert_call_once, system::*};

pub struct Arch;

impl Arch {
    pub unsafe fn init() {
        assert_call_once!();

        todo!();
    }
}
