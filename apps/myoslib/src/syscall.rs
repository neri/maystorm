// myos system calls

use myosabi::svc::Function;

#[link(wasm_import_module = "arl")]
extern "C" {
    pub fn svc0(_: Function) -> usize;
    pub fn svc1(_: Function, _: usize) -> usize;
    pub fn svc2(_: Function, _: usize, _: usize) -> usize;
    pub fn svc3(_: Function, _: usize, _: usize, _: usize) -> usize;
    pub fn svc4(_: Function, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn svc5(_: Function, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn svc6(_: Function, _: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
}

/// Display a string
#[inline]
pub fn os_print(s: &str) {
    unsafe {
        svc2(Function::PrintString, s.as_ptr() as usize, s.len());
    }
}

/// Create a new window
#[inline]
pub fn os_new_window(s: &str, width: usize, height: usize) -> usize {
    unsafe {
        svc4(
            Function::NewWindow,
            s.as_ptr() as usize,
            s.len(),
            width,
            height,
        )
    }
}

/// Draw a string in a window
#[inline]
pub fn os_draw_text(window: usize, x: usize, y: usize, s: &str, color: u32) {
    unsafe {
        svc6(
            Function::DrawText,
            window,
            x,
            y,
            s.as_ptr() as usize,
            s.len(),
            color as usize,
        );
    }
}

/// Fill a rectangle in a window
#[inline]
pub fn os_fill_rect(window: usize, x: usize, y: usize, width: usize, height: usize, color: u32) {
    unsafe {
        svc6(
            Function::FillRect,
            window,
            x,
            y,
            width,
            height,
            color as usize,
        );
    }
}

/// Wait for key input
#[inline]
pub fn os_wait_key(window: usize) -> u32 {
    unsafe { svc1(Function::WaitKey, window) as u32 }
}

/// Draw a bitmap in a window
#[inline]
pub fn os_blt8(window: usize, x: usize, y: usize, bitmap: usize) {
    unsafe {
        svc4(Function::Blt8, window, x, y, bitmap);
    }
}

/// Draw a bitmap in a window
#[inline]
pub fn os_blt1(window: usize, x: usize, y: usize, bitmap: usize, color: u32, scale: isize) {
    unsafe {
        svc6(
            Function::Blt1,
            window,
            x,
            y,
            bitmap,
            color as usize,
            scale as usize,
        );
    }
}

/// Reflect the window's bitmap to the screen now.
#[inline]
pub fn os_flash_window(window: usize) {
    unsafe {
        svc1(Function::FlashWindow, window);
    }
}

/// Get the value of the monotonic timer in microseconds
#[inline]
pub fn os_monotonic() -> u32 {
    unsafe { svc0(Function::Monotonic) as u32 }
}

/// Blocks a thread for the specified microseconds.
#[inline]
pub fn os_usleep(us: u32) {
    unsafe {
        svc1(Function::Usleep, us as usize);
    }
}

/// Return a random number
#[inline]
pub fn os_rand() -> u32 {
    unsafe { svc0(Function::Rand) as u32 }
}

#[inline]
pub fn os_srand(srand: u32) -> u32 {
    unsafe { svc1(Function::Srand, srand as usize) as u32 }
}

#[inline]
pub fn os_version() -> u32 {
    unsafe { svc1(Function::GetVersion, 0) as u32 }
}

#[inline]
pub fn os_alloc(size: usize, align: usize) -> usize {
    unsafe { svc2(Function::Alloc, size, align) }
}

#[inline]
pub fn os_free(ptr: usize) {
    unsafe {
        svc1(Function::Free, ptr);
    }
}
