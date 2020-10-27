# My first OS

My first OS written in Rust.

## Requirements

* 64bit UEFI 2.X / ACPI 2.X
* x64 processor with up to 64 cores
* XX MB of system memory
* 800x600 pixels screen
* PS/2 keyboard and mouse
* HPET
* XHCI (in the future)

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
