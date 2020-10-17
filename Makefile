.PHONY: love all clean install run runs test

RUST_ARCH	= x86_64-unknown-uefi
EFI_ARCH	= x64
MNT			= ./mnt
EFI_BOOT	= $(MNT)/efi/boot
KERNEL_BIN	= $(EFI_BOOT)/kernel.bin
BOOT_EFI	= $(EFI_BOOT)/boot$(EFI_ARCH).efi
KERNEL_TARGET	= target/$(RUST_ARCH)/release/kernel.efi
BOOT_EFI_TARGET	= target/$(RUST_ARCH)/release/boot-efi.efi
TARGETS		= $(KERNEL_TARGET) $(BOOT_EFI_TARGET)
BUILD		= rustup run nightly cargo build -Z build-std --release --target $(RUST_ARCH).json
OVMF		= var/ovmfx64.fd
URL_OVMF	= https://github.com/retrage/edk2-nightly/raw/master/bin/RELEASEX64_OVMF.fd

all: $(TARGETS)

clean:
	-rm -rf target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(KERNEL_TARGET): src/* src/**/* src/**/**/* src/**/**/**/* src/**/**/**/**/*
	$(BUILD)

$(BOOT_EFI_TARGET): src/* src/**/* src/**/**/* src/**/**/**/* src/**/**/**/**/*
	$(BUILD)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(OVMF):
	-mkdir -p var
	curl -# -L -o $@ $(URL_OVMF)

run: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -rtc base=localtime,clock=host -monitor stdio -device nec-usb-xhci,id=xhci

runs: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -nographic

install: $(KERNEL_BIN) $(BOOT_EFI)

$(KERNEL_BIN): $(KERNEL_TARGET) $(EFI_BOOT)
	cp $< $@

$(BOOT_EFI): $(BOOT_EFI_TARGET) $(EFI_BOOT)
	cp $< $@

