# MEG-OS codename Maystorm

My first hobby OS written in Rust, one version of which is about 20,000 lines of code and supports multitasking, windows, WebAssembly runtime, and simple applications.

## Feature

* A hobby OS written in Rust
* Not a POSIX clone system
* 64bit OS booting with UEFI
* Multi-core support for up to 64 cores
* Support for WebAssembly

## Requirements

* 64bit UEFI 2.X / ACPI 2.X
* x64 processor with up to 64 cores, required features: NX RDTSCP RDRAND
* ??? MB of system memory
* 800x600 pixels screen
* PS/2 keyboard and mouse
* HPET

## Build Environment

* Rust nightly
* nasm
* llvm (ld.lld)
* qemu + ovmf (optional)

### how to build

1. Install llvm
2. Install rust (nightly)
3. `make apps`
4. `make install`

If you get an error that the linker cannot be found, configure your linker in `~/.cargo/config.toml` or something similar.

```
[target.x86_64-unknown-none]
linker = "/opt/homebrew/opt/llvm/bin/ld.lld"
```

### how to run on qemu

```
$ make run
```

## HOE: Haribote-OS Emulation Subsystem

* We have confirmed that about half of the apps work at this point. Some APIs are not yet implemented.
* This subsystem may not be supported in the future, or its architecture may change.

## History

### 2020-05-09

* Initial Commit

## License

MIT License

&copy; 2020 Nerry.
