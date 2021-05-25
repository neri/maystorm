// MEG-OS benchmark
#![no_main]
#![no_std]
#![feature(asm)]

use core::fmt::Write;
use megoslib::*;

// The number of instruction steps below is 12
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
const BENCH_STEPS: u64 = 12;
const BENCH_COUNT1: u64 = 100_000;

#[no_mangle]
fn _start() {
    {
        println!("Benchmarking...");

        let mut mips_temp: usize = 0;
        let time = os_bench(|| {
            for _ in 0..BENCH_COUNT1 {
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
        let steps: u64 = BENCH_STEPS * BENCH_COUNT1;
        let (bogomips, multi) = if steps < time {
            ((steps * 1_000_000).checked_div(time).unwrap_or(0), 1)
        } else {
            let mut mips_temp: usize = 0;
            let multi = steps / time;
            let count = BENCH_COUNT1 * multi;
            let time = os_bench(|| {
                for _ in 0..count {
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
            let steps: u64 = BENCH_STEPS * count;
            ((steps * 1_000_000).checked_div(time).unwrap_or(0), multi)
        };
        let bogokips = bogomips.checked_div(1000).unwrap_or(0) as u32;
        let kips = bogokips.checked_rem(1000).unwrap_or(0);
        let mips = bogokips.checked_div(1000).unwrap_or(0);
        println!("result: {}.{:03} bogomips (x{})", mips, kips, multi);
    }
}
