// MyOS benchmark
#![no_main]
#![no_std]
#![feature(asm)]

use core::fmt::Write;
use myoslib::*;

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
const BENCH_STEPS: u64 = 13;
const BENCH_COUNT1: u64 = 100_000;
const BENCH_COUNT2: u64 = 1_000_000;

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
        if steps < time {
            let bogomips = (steps * 1_000_000).checked_div(time).unwrap_or(0);
            println!("result: {} bogomips", bogomips as usize);
        } else {
            let mut mips_temp: usize = 0;
            let time = os_bench(|| {
                for _ in 0..BENCH_COUNT2 {
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
            let steps: u64 = BENCH_STEPS * BENCH_COUNT2;
            let bogomips = (steps * 1_000_000).checked_div(time).unwrap_or(0);
            println!("result: {} bogomips", bogomips as usize);
        }
    }
}
