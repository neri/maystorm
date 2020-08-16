.PHONY: love all clean run install rust

RUST_ARCH	= x86_64-unknown-uefi
BOOT_EFI	= BOOTX64.EFI
MNT			= ./mnt
EFI_BOOT	= $(MNT)/EFI/BOOT
OVMF		= var/ovmfx64.fd
URL_OVMF	= https://github.com/retrage/edk2-nightly/raw/master/bin/RELEASEX64_OVMF.fd
KERNEL_TARGET	= target/$(RUST_ARCH)/release/kernel.efi
BOOT_TARGET	= target/$(RUST_ARCH)/release/boot.efi
KERNEL_EXECUTE	= $(EFI_BOOT)/kernel.bin
BOOT_EXECUTE	= $(EFI_BOOT)/$(BOOT_EFI)
TARGETS		= $(KERNEL_TARGET) $(BOOT_TARGET)
# BUILD		= rustup run nightly cargo xbuild --release --target $(RUST_ARCH)
BUILD		= rustup run nightly cargo build -Z build-std --release --target $(RUST_ARCH)

all: $(TARGETS)

clean:
	-rm -rf target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(KERNEL_TARGET): src/* src/**/* src/**/**/* src/**/**/**/* src/**/**/**/**/*
	$(BUILD)

$(BOOT_TARGET): src/* src/**/* src/**/**/* src/**/**/**/* src/**/**/**/**/*
	$(BUILD)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(KERNEL_EXECUTE): $(KERNEL_TARGET) $(EFI_BOOT)
	cp $< $@

$(BOOT_EXECUTE): $(BOOT_TARGET) $(EFI_BOOT)
	cp $< $@

install: $(KERNEL_EXECUTE) $(BOOT_EXECUTE)

$(OVMF):
	-mkdir -p var
	curl -# -L -o $@ $(URL_OVMF)

run: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -monitor stdio

runs: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -nographic

test: install
	cp $(KERNEL_EXECUTE) /Volumes/EFI_TEST/EFI/MEGOS/BOOTX64.EFI
