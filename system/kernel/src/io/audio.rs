use crate::sync::Mutex;
use alloc::boxed::Box;

static mut BEEP_DRIVER: Mutex<Option<Box<dyn BeepDriver>>> = Mutex::new(None);

pub struct AudioManager {}

impl AudioManager {
    #[inline]
    pub unsafe fn set_beep_driver(driver: Box<dyn BeepDriver>) {
        *BEEP_DRIVER.lock().unwrap() = Some(driver);
    }

    pub fn make_beep(mhz: usize) {
        unsafe {
            if let Some(driver) = BEEP_DRIVER.lock().unwrap().as_ref() {
                driver.make_beep(mhz);
            }
        }
    }
}

pub trait BeepDriver {
    fn make_beep(&self, mhz: usize);
}
