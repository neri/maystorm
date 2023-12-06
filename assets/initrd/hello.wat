;; minimal hello world for meg-os
(module
  (import "megos-canary" "svc2" (func $svc2 (param i32 i32 i32) (result i32)))
  (func $_start (export "_start")
    i32.const 1
    i32.const 16
    i32.const 13
    call $svc2
    drop
  )
  (memory 1)
  (data $.rodata (i32.const 16) "hello, world\0a")
)
