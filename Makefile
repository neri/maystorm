.PHONY: all clean run install love

RUST_ARCH	= x86_64-unknown-uefi
MNT			= ./mnt
EFI_BOOT	= $(MNT)/EFI/BOOT
EXECUTABLE	= $(EFI_BOOT)/BOOTX64.EFI
TARGET		= target/$(RUST_ARCH)/release/uefi-pg.efi
OVMF		= var/ovmfx64.fd
URL_OVMF	= https://github.com/retrage/edk2-nightly/raw/master/bin/RELEASEX64_OVMF.fd

all: $(TARGET)

clean:
	-rm -rf target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(TARGET): src/* src/**/* src/**/**/* src/**/**/**/*
	rustup run nightly cargo xbuild --release --target $(RUST_ARCH)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EXECUTABLE): $(TARGET) $(EFI_BOOT)
	cp $< $@

install: $(EXECUTABLE)

$(OVMF):
	-mkdir -p var
	curl -# -L -o $@ $(URL_OVMF)

run: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -monitor stdio
