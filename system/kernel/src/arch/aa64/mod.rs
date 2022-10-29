mod hal_aa64;
pub use hal_aa64::*;

use crate::{check_once_call, system::*};

pub struct Arch;

impl Arch {
    pub unsafe fn init() {
        check_once_call!();

        todo!();
    }
}
