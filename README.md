# My OS

My first toy OS written in Rust.

## Feature

* A toy OS written in Rust
* 64bit OS booting with UEFI
* Multi-core support for up to 64 cores
* Support for WebAssembly
* Built-in Haribote-OS emulator

## Requirements

* 64bit UEFI 2.X / ACPI 2.X
* x64 processor with up to 64 cores, required features: NX RDTSCP RDRAND
* ??? MB of system memory
* 800x600 pixels screen
* PS/2 keyboard and mouse
* HPET
* XHCI (in the future)

## Haribote-OS emulator

* We have confirmed that about half of the apps work at this point. Some APIs are not yet implemented.
* Window and timer handles are task-specific. They are set to the same initial value each time a task is launched. They are not visible to other tasks.
* The drawing mechanism is different, so I converted it when it was displayed. Frequent re-drawing may slow it down.

## Build Environment

* Rust nightly
* nasm
* qemu + ovmf (optional)

### how to build

```
$ make
```

### how to run

```
$ make run
```

## History

### 2020-05-09

* Initial Commit

## License

MIT License

&copy; 2020 Nerry.
