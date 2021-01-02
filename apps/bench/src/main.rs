// MyOS benchmark
#![no_main]
#![no_std]
#![feature(asm)]

use core::fmt::Write;
use myoslib::*;

const BENCH_STEPS: u64 = 1_000_000;

#[no_mangle]
fn _start() {
    {
        let mut mips_temp: usize = 0;
        let time = os_bench(|| {
            for _ in 0..BENCH_STEPS {
                unsafe {
                    asm!("
                    local.get {0}
                    i32.const 1
                    i32.add
                    local.set {0}
                    ", inlateout(local) mips_temp);
                }
            }
        }) as u64;

        // The number of instruction steps below is 13
        // loop  ;; label = @2
        //   local.get 2
        //   i64.eqz
        //   br_if 1 (;@1;)
        //   local.get 2
        //   i64.const -1
        //   i64.add
        //   local.set 2
        //   local.get 1
        //   i32.const 1
        //   i32.add
        //   local.set 1
        //   br 0 (;@2;)
        let steps: u64 = 13 * BENCH_STEPS;

        let bogomips = steps * 1_000_000 / time;
        println!("{} bogomips", bogomips);
    }
}
