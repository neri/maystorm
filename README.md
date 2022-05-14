# MEG-OS

A hobby operating system written in Rust that supports WebAssembly.

* [Documentation](https://meg-os.github.io/maystorm/kernel/)

## Feature

* A hobby operating system written in Rust
* Not a POSIX clone system
* Supports applications in WebAssembly format

## Requirements

### UEFI PC Platform

* 64bit UEFI v2.X+
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

### build

1. Install llvm
2. Install rust (nightly)
3. `make install`

If you get an error that the linker cannot be found, configure your linker in `~/.cargo/config.toml` or something similar.

```
[target.x86_64-unknown-none]
linker = "/opt/homebrew/opt/llvm/bin/ld.lld"
```

### run on qemu

1. Copy qemu's OVMF for x64 to `var/ovmfx64.fd`.
2. Follow the build instructions to finish the installation.
3. `make run`

### run on real hardware

* Copy the files in the path `mnt/efi` created by the build to a USB memory stick and reboot your computer.
* You may need to change settings such as SecureBoot.

## HOE: Haribote-OS Emulation Subsystem

* We have confirmed that about half of the apps work at this point. Some APIs are not yet implemented.
* This subsystem may be unsupported in the future, or may be replaced by another implementation.

## History

### 2020-05-09

* Initial Commit

## LICENSE

MIT License

&copy; 2020,2021,2022 MEG-OS Project.

### Wall paper

* CC BY-SA 4.0 &copy; 猫(1010) 

## Contributors

### Kernel

[![Nerry](https://github.com/neri.png?size=50)](https://github.com/neri "Nerry")

### Wall paper

[![猫(1010)](https://github.com/No000.png?size=50)](https://github.com/No000 "猫(1010)")
