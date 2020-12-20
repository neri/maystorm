.PHONY: love all clean install iso run runs test apps

RUST_ARCH	= x86_64-unknown-uefi
EFI_ARCH	= x64
MNT			= ./mnt
EFI_BOOT	= $(MNT)/efi/boot
KERNEL_BIN	= $(EFI_BOOT)/kernel.bin
BOOT_EFI	= $(EFI_BOOT)/boot$(EFI_ARCH).efi
INITRD_IMG	= $(EFI_BOOT)/initrd.img
TARGET_KERNEL	= sys/target/$(RUST_ARCH)/release/kernel.efi
TARGET_BOOT_EFI	= sys/target/$(RUST_ARCH)/release/boot-efi.efi
TARGET_ISO	= var/myos.iso
TARGETS		= $(TARGET_KERNEL) $(TARGET_BOOT_EFI)
BUILD		= (cd sys; cargo build -Zbuild-std --release --target $(RUST_ARCH).json)
OVMF		= var/ovmfx64.fd

all: $(TARGETS)

clean:
	-rm -rf target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(TARGET_KERNEL): sys/kernel/* sys/kernel/**/* sys/kernel/**/**/* sys/kernel/**/**/**/* sys/kernel/**/**/**/**/*
	$(BUILD)

$(TARGET_BOOT_EFI): sys/boot-efi/* sys/boot-efi/**/* sys/boot-efi/**/**/* sys/boot-efi/**/**/**/* sys/boot-efi/**/**/**/**/*
	$(BUILD)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

run: install $(OVMF)
	qemu-system-x86_64 -cpu max -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -rtc base=localtime,clock=host -monitor stdio -device nec-usb-xhci,id=xhci

runs: install $(OVMF)
	qemu-system-x86_64 -cpu max -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -nographic

install: $(KERNEL_BIN) $(BOOT_EFI)
	mcopy -D o -i $(INITRD_IMG) apps/target/wasm32-unknown-unknown/release/*.wasm ::

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

$(KERNEL_BIN): $(TARGET_KERNEL) $(EFI_BOOT)
	cp $< $@

$(BOOT_EFI): $(TARGET_BOOT_EFI) $(EFI_BOOT)
	cp $< $@

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release
