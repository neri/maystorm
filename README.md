# MEG-OS codename Maystorm

My first hobby OS written in Rust, one version of which is about 20,000 lines of code and supports multitasking, windows, WebAssembly runtime, and simple applications.

* [Documentation](https://meg-os.github.io/maystorm)

## Feature

* A hobby OS written in Rust
* Not a POSIX clone system
* Support for WebAssembly

## Requirements

### UEFI PC Platform

* 64bit UEFI v2.X+ / ACPI v2.X+
* x64 processor with up to 64 cores
* ??? MB of system memory
* 800 x 600 pixel resolution
* SMBIOS v2.X+ (optional)

### Legacy Platform (Not yet included in this repository)

* IBM PC compatible / 日本電気 PC-9800 ｼﾘｰｽﾞ ﾊﾟｰｿﾅﾙ ｺﾝﾋﾟｭｰﾀ / 富士通 FM TOWNS
* 486SX or later
* 3.6MB? or a lot more memory
* VGA or better video adapter
  * 640 x 480 pixel resolution
  * 256 color mode

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
