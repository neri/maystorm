# My practice OS

My practice OS written in Rust

## Operating Requirements

* 64bit UEFI 2.X + ACPI 2.X
* Up to 64 logical processor cores
* XX MB of system memory
* 800x600 pixels graphics display
* PS/2 Keyboard and mouse
* XHCI (in the future)

## Requirements to Build

* Rust nightly
* qemu + ovmf (optional)

### how to build

```
$ make
```

### how to run

```
$ make run
```

## License

MIT License

&copy; 2020 Nerry.
