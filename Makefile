.PHONY: all clean run install love

RUST_ARCH	= x86_64-unknown-uefi
BOOT_EFI	= BOOTX64.EFI
MNT			= ./mnt
EFI_BOOT	= $(MNT)/EFI/BOOT
OVMF		= var/ovmfx64.fd
URL_OVMF	= https://github.com/retrage/edk2-nightly/raw/master/bin/RELEASEX64_OVMF.fd
KERNEL_TARGET	= target/$(RUST_ARCH)/release/kernel.efi
KERNEL_EXECUTE	= $(EFI_BOOT)/$(BOOT_EFI)
TARGETS		= $(KERNEL_TARGET)

all: $(TARGETS)

clean:
	-rm -rf target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(KERNEL_TARGET): src/* src/**/* src/**/**/* src/**/**/**/*
	rustup run nightly cargo xbuild --release --target $(RUST_ARCH)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(KERNEL_EXECUTE): $(KERNEL_TARGET) $(EFI_BOOT)
	cp $< $@

install: $(KERNEL_EXECUTE)

$(OVMF):
	-mkdir -p var
	curl -# -L -o $@ $(URL_OVMF)

run: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -monitor stdio

test: install
	cp mnt/EFI/BOOT/BOOTX64.EFI /Volumes/EFI_TEST/EFI/MEGOS/BOOTX64.EFI
