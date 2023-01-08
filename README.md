# MEG-OS

![GitHub](https://img.shields.io/github/license/neri/maystorm) ![GitHub top language](https://img.shields.io/github/languages/top/neri/maystorm)

A hobby operating system written in Rust that supports WebAssembly.

* [Documentation for megstd](https://neri.github.io/maystorm/megstd/)

## Feature

* A hobby operating system written in Rust
* Not a POSIX clone system
* Supports applications in WebAssembly format

## Requirements

### IBM PC Compatibles in the 2020s

* UEFI v2.X+
* ACPI v2.X+
* SMBIOS v2.X+ (optional)
* x64 processor with up to 64 cores
* ??? GB of system memory
* 800 x 600 pixels or higher resolution
* XHCI (optional)
* HD Audio (optional)

## Build Environment

* Rust nightly
* nasm
* llvm (ld.lld)
* qemu + ovmf (optional)

### Minimum supported Rust version

The latest version is recommended whenever possible.

### building

1. `make install`

If you get a linker error, configure your linker in `~/.cargo/config.toml` or similar.

```
[target.x86_64-unknown-none]
linker = "/opt/homebrew/opt/llvm/bin/ld.lld"
```

### run on qemu

1. Follow the build instructions to finish the installation.
2. Copy qemu's OVMF for x64 to `var/ovmfx64.fd`.
3. `make run`

### run on real hardware

1. Follow the build instructions to finish the installation.
2.  Copy the files in the path `mnt/efi` created by the build to a USB memory stick and reboot your computer.
* You may need to change settings such as SecureBoot.

## HOE: Haribote-OS Emulation Subsystem

* We have confirmed that about half of the apps work at this point. Some APIs are not yet implemented.
* This subsystem may be unsupported in the future, or may be replaced by another implementation.
* If the haribote application is launched with insufficient 32-bit memory, it will not operate properly.

## History

### 2020-05-09

* Initial Commit

## LICENSE

MIT License

&copy; 2020-2023 MEG-OS Project.

### Wall paper

* CC BY-SA 4.0 &copy; 猫(1010) 

## Contributors

### Kernel

[![Nerry](https://github.com/neri.png?size=50)](https://github.com/neri "Nerry")

### Wall paper

[![猫(1010)](https://github.com/No000.png?size=50)](https://github.com/No000 "猫(1010)")
