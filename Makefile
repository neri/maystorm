.PHONY: all clean run install love

RUST_ARCH	= x86_64-unknown-uefi
MNT			= ./mnt
EFI_BOOT	= $(MNT)/EFI/BOOT
EXECUTABLE	= $(EFI_BOOT)/BOOTX64.EFI
TARGET		= target/$(RUST_ARCH)/release/uefi-pg.efi
OVMF		= var/ovmfx64.fd

all: $(TARGET)

clean:
	-rm -rf target

$(TARGET): src/* src/**/* src/**/**/*
	rustup run nightly cargo xbuild --target $(RUST_ARCH) --release

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EXECUTABLE): $(TARGET) $(EFI_BOOT)
	cp $< $@

install: $(EXECUTABLE)

run: install $(OVMF)
	qemu-system-x86_64 -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -monitor stdio
