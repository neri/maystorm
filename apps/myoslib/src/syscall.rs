// myos system calls

extern "C" {
    pub fn syscall0(_: usize) -> usize;
    pub fn syscall1(_: usize, _: usize) -> usize;
    pub fn syscall2(_: usize, _: usize, _: usize) -> usize;
    pub fn syscall3(_: usize, _: usize, _: usize, _: usize) -> usize;
    pub fn syscall4(_: usize, _: usize, _: usize, _: usize, _: usize) -> usize;
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
