// myos system calls

#[link(wasm_import_module = "arl")]
extern "C" {
    pub fn svc0(_: usize) -> usize;
    pub fn svc1(_: usize, _: usize) -> usize;
    pub fn svc2(_: usize, _: usize, _: usize) -> usize;
    pub fn svc3(_: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn svc4(_: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn svc5(_: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn svc6(_: usize, _: usize, _: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
}

#[inline]
pub fn os_print(s: &str) {
    unsafe {
        svc2(1, s.as_ptr() as usize, s.len());
    }
}

#[inline]
pub fn os_new_window(s: &str, width: usize, height: usize) -> usize {
    unsafe { svc4(3, s.as_ptr() as usize, s.len(), width, height) }
}

#[inline]
pub fn os_draw_text(handle: usize, x: usize, y: usize, s: &str, color: u32) {
    unsafe {
        svc6(
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
        svc6(5, handle, x, y, width, height, color as usize);
    }
}

#[inline]
pub fn os_wait_key(handle: usize) -> u32 {
    unsafe { svc1(6, handle) as u32 }
}

#[inline]
pub fn os_blt1(handle: usize, x: usize, y: usize, bitmap: usize) {
    unsafe {
        svc4(7, handle, x, y, bitmap);
    }
}

#[inline]
pub fn os_rand() -> u32 {
    unsafe { svc0(50) as u32 }
}

#[inline]
pub fn os_alloc(size: usize, align: usize) -> usize {
    unsafe { svc2(100, size, align) }
}

#[inline]
pub fn os_free(ptr: usize) {
    unsafe {
        svc1(101, ptr);
    }
}
