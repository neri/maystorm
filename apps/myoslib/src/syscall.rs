// myos system calls

#[link(wasm_import_module = "arl")]
extern "C" {
    pub fn syscall0(_: usize) -> usize;
    pub fn syscall1(_: usize, _: usize) -> usize;
    pub fn syscall2(_: usize, _: usize, _: usize) -> usize;
    pub fn syscall3(_: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn syscall4(_: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn syscall5(_: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn syscall6(_: usize, _: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
}

#[inline]
pub fn os_print(s: &str) {
    unsafe {
        syscall2(1, s.as_ptr() as usize, s.len());
    }
}

#[inline]
pub fn os_new_window(s: &str, width: usize, height: usize) -> usize {
    unsafe { syscall4(3, s.as_ptr() as usize, s.len(), width, height) }
}

#[inline]
pub fn os_draw_text(handle: usize, x: usize, y: usize, s: &str, color: u32) {
    unsafe {
        syscall6(
            4,
            handle,
            x,
            y,
            s.as_ptr() as usize,
            s.len(),
            color as usize,
        );
    }
}

#[inline]
pub fn os_fill_rect(handle: usize, x: usize, y: usize, width: usize, height: usize, color: u32) {
    unsafe {
        syscall6(5, handle, x, y, width, height, color as usize);
    }
}

#[inline]
pub fn os_wait_key(handle: usize) -> u32 {
    unsafe { syscall1(6, handle) as u32 }
}
