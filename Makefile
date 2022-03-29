.PHONY: love default all clean install iso run runs test apps doc kernel boot
.SUFFIXED: .wasm

EFI_ARCH	= x86_64-unknown-uefi
KRNL_ARCH	= x86_64-unknown-none
EFI_SUFFIX	= x64
MNT			= ./mnt/
MISC		= ./misc/
ASSETS		= ./assets/
EFI_BOOT	= $(MNT)efi/boot
EFI_VENDOR	= $(MNT)efi/megos
KERNEL_BIN	= $(EFI_VENDOR)/kernel.bin
BOOT_EFI1	= $(EFI_BOOT)/boot$(EFI_SUFFIX).efi
BOOT_EFI2	= $(EFI_VENDOR)/boot$(EFI_SUFFIX).efi
INITRD_IMG	= $(EFI_VENDOR)/initrd.img
TARGET_KERNEL	= system/target/$(KRNL_ARCH)/release/kernel
TARGET_BOOT_EFI	= boot/target/$(EFI_ARCH)/release/boot-efi.efi
TARGET_ISO	= var/megos.iso
TARGETS		= boot kernel
ALL_TARGETS	= $(TARGETS) apps
OVMF		= var/ovmfx64.fd
INITRD_FILES	= LICENSE $(ASSETS)initrd/* apps/target/wasm32-unknown-unknown/release/*.wasm

default: $(TARGETS)

all: $(ALL_TARGETS)

clean:
	-rm -rf system/target apps/target boot/target tools/target

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EFI_VENDOR):
	mkdir -p $(EFI_VENDOR)

run:
	qemu-system-x86_64 -machine q35 \
		-cpu Haswell -smp 4,cores=2,threads=2 \
		-bios $(OVMF) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci -device usb-tablet \
		-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
		-device intel-hda -device hda-duplex \
		-monitor stdio

run_leg:
	qemu-system-x86_64 -machine q35 \
		-cpu Haswell -smp 4,cores=2,threads=2 \
		-bios $(OVMF) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci \
		-drive format=raw,file=fat:rw:$(MNT) \
		-device intel-hda -device hda-duplex \
		-monitor stdio

run_up:
	qemu-system-x86_64 -machine q35 \
		-cpu IvyBridge \
		-bios $(OVMF) \
		-rtc base=localtime,clock=host \
		-device nec-usb-xhci,id=xhci -device usb-tablet \
		-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
		-device intel-hda -device hda-duplex \
		-monitor stdio

boot:
	(cd boot; cargo build -Zbuild-std --release --target $(EFI_ARCH).json)

kernel:
	(cd system; cargo build -Zbuild-std --release --target $(KRNL_ARCH).json)

install: $(EFI_VENDOR) $(EFI_BOOT) $(ALL_TARGETS) tools/mkinitrd/src/*.rs
	cp $(TARGET_BOOT_EFI) $(BOOT_EFI1)
	cp $(TARGET_BOOT_EFI) $(BOOT_EFI2)
	cp $(TARGET_KERNEL) $(KERNEL_BIN)
	cargo run --manifest-path ./tools/mkinitrd/Cargo.toml -- $(INITRD_IMG) $(INITRD_FILES)

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release
	for name in ./apps/target/wasm32-unknown-unknown/release/*.wasm; do \
	cargo run --manifest-path ./tools/wasm-strip/Cargo.toml -- -preserve name $$name $$name; done

test:
	cargo test --manifest-path lib/wasm/Cargo.toml

doc:
	(cd system; cargo doc --all --target $(KRNL_ARCH).json)
