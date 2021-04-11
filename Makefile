.PHONY: love all clean install iso run runs test apps

KRNL_ARCH	= x86_64-unknown-uefi
EFI_ARCH	= x86_64-unknown-uefi
EFI_SUFFIX	= x64
MNT			= ./mnt/
MISC		= ./misc/
EFI_BOOT	= $(MNT)efi/boot
EFI_VENDOR	= $(MNT)efi/megos
KERNEL_BIN	= $(EFI_VENDOR)/kernel.bin
BOOT_EFI1	= $(EFI_BOOT)/boot$(EFI_SUFFIX).efi
BOOT_EFI2	= $(EFI_VENDOR)/boot$(EFI_SUFFIX).efi
INITRD_IMG	= $(EFI_VENDOR)/initrd.img
TARGET_KERNEL	= sys/target/$(KRNL_ARCH)/release/kernel.efi
TARGET_BOOT_EFI	= boot/target/$(EFI_ARCH)/release/boot-efi.efi
TARGET_ISO	= var/myos.iso
TARGETS		= $(TARGET_KERNEL) $(TARGET_BOOT_EFI)
OVMF		= var/ovmfx64.fd
INITRD_FILES	= $(MISC)initrd/* apps/target/wasm32-unknown-unknown/release/*.wasm

all: $(TARGETS)

clean:
	-rm -rf sys/target apps/target boot/target tools/target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(TARGET_KERNEL): sys/kernel/* sys/kernel/**/* sys/kernel/**/**/* sys/kernel/**/**/**/* sys/kernel/**/**/**/**/* lib/**/src/**/*.rs lib/**/src/**/**/*.rs
	(cd sys; cargo build -Zbuild-std --release --target $(KRNL_ARCH).json)

$(TARGET_BOOT_EFI): boot/boot-efi/* boot/boot-efi/src/* boot/boot-efi/src/**/*
	(cd boot; cargo build -Zbuild-std --release --target $(EFI_ARCH).json)

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EFI_VENDOR):
	mkdir -p $(EFI_VENDOR)

run: install $(OVMF)
	qemu-system-x86_64 -cpu max -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -rtc base=localtime,clock=host -monitor stdio -device nec-usb-xhci,id=xhci

# runs: install $(OVMF)
# 	qemu-system-x86_64 -cpu max -smp 4 -bios $(OVMF) -drive format=raw,file=fat:rw:$(MNT) -nographic

install: $(KERNEL_BIN) $(BOOT_EFI1) tools/mkinitrd/src/*.rs $(INITRD_FILES) apps
	cargo run --manifest-path ./tools/mkinitrd/Cargo.toml -- $(INITRD_IMG) $(INITRD_FILES)

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

$(KERNEL_BIN): $(TARGET_KERNEL) $(EFI_VENDOR)
	cp $< $@

$(BOOT_EFI1): $(TARGET_BOOT_EFI) $(EFI_BOOT) $(EFI_VENDOR)
	cp $< $@
	cp $< $(BOOT_EFI2)

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release

test:
	cargo test --manifest-path lib/wasm/Cargo.toml
