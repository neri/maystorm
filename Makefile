.PHONY: love default all clean install iso run runs test apps doc kernel boot refresh
.SUFFIXES: .wasm

MNT			= ./mnt/
MISC		= ./misc/
ASSETS		= ./assets/

EFI_BOOT	= $(MNT)efi/boot
EFI_VENDOR	= $(MNT)efi/megos
KERNEL_BIN	= $(EFI_VENDOR)/kernel.bin
INITRD_IMG	= $(EFI_VENDOR)/initrd.img

KRNL_ARCH	= x86_64-unknown-none
TARGET_KERNEL	= system/target/$(KRNL_ARCH)/release/kernel.bin
TARGET_ISO	= var/megos.iso
TARGETS		= boot kernel
ALL_TARGETS	= $(TARGETS) apps
VAR_INITRD	= var/initrd/
INITRD_DEV	= var/initrd/dev/
INITRD_FILES	= LICENSE $(VAR_INITRD)* $(ASSETS)initrd/* apps/target/wasm32-unknown-unknown/release/*.wasm

X64_SMP			= system/src/arch/x64/smpinit
X64_SMP_ASM		= $(X64_SMP).asm
X64_SMP_BIN		= $(X64_SMP).bin

QEMU_X64		= qemu-system-x86_64
OVMF_X64		= var/ovmfx64.fd
BOOT_EFI_BOOT1	= $(EFI_BOOT)/bootx64.efi
BOOT_EFI_VENDOR1	= $(EFI_VENDOR)/bootx64.efi
TARGET_BOOT_EFI1	= boot/target/x86_64-unknown-uefi/release/boot-efi.efi

OVMF_X86		= var/ovmfia32.fd
BOOT_EFI_BOOT2	= $(EFI_BOOT)/bootia32.efi
BOOT_EFI_VENDOR2	= $(EFI_VENDOR)/bootia32.efi
TARGET_BOOT_EFI2	= boot/target/i686-unknown-uefi/release/boot-efi.efi

default: $(TARGETS)

all: $(ALL_TARGETS)

clean:
	-rm -rf system/target apps/target boot/target tools/target lib/*/target

refresh: clean
	-rm system/Cargo.lock apps/Cargo.lock boot/Cargo.lock tools/Cargo.lock lib/*/Cargo.lock

# $(RUST_ARCH).json:
# 	rustc +nightly -Z unstable-options --print target-spec-json --target $(RUST_ARCH) | sed -e 's/-sse,+/+sse,-/' > $@

$(EFI_BOOT):
	mkdir -p $(EFI_BOOT)

$(EFI_VENDOR):
	mkdir -p $(EFI_VENDOR)

run:
	$(QEMU_X64) -machine q35 -cpu SandyBridge -smp 4,cores=2,threads=2 \
-bios $(OVMF_X64) \
-rtc base=localtime,clock=host \
-device virtio-net-pci \
-device nec-usb-xhci,id=xhci \
-device intel-hda -device hda-duplex \
-device usb-hub,bus=xhci.0,port=1,id=usb-hub \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,bus=xhci.0,port=2,drive=stick \
-device usb-tablet \
-device usb-audio,id=usb-audio \
-serial mon:stdio

run_up:
	$(QEMU_X64) -machine q35 -cpu IvyBridge \
-bios $(OVMF_X64) \
-rtc base=localtime,clock=host \
-device nec-usb-xhci,id=xhci -device usb-tablet \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
-device intel-hda -device hda-duplex \
-monitor stdio

run_x86:
	$(QEMU_X64) -machine q35 -cpu IvyBridge -smp 4,cores=2,threads=2 \
-bios $(OVMF_X86) \
-rtc base=localtime,clock=host \
-device nec-usb-xhci,id=xhci -device usb-tablet \
-drive if=none,id=stick,format=raw,file=fat:rw:$(MNT) -device usb-storage,drive=stick \
-device intel-hda -device hda-duplex \
-monitor stdio

boot:
	(cd boot; cargo build --release --target x86_64-unknown-uefi --target i686-unknown-uefi)

kernel: $(X64_SMP_BIN)
	(cd system; cargo build --release --target $(KRNL_ARCH).json)

$(X64_SMP_BIN): $(X64_SMP_ASM)
	nasm -f bin $< -o $@

$(VAR_INITRD):
	-mkdir -p $(INITRD_DEV)

install: test $(EFI_VENDOR) $(EFI_BOOT) $(ALL_TARGETS) tools/mkinitrd/src/*.rs $(VAR_INITRD)
	cp $(TARGET_BOOT_EFI1) $(BOOT_EFI_BOOT1)
	cp $(TARGET_BOOT_EFI1) $(BOOT_EFI_VENDOR1)
	cp $(TARGET_BOOT_EFI2) $(BOOT_EFI_BOOT2)
	cp $(TARGET_BOOT_EFI2) $(BOOT_EFI_VENDOR2)
	cp $(TARGET_KERNEL) $(KERNEL_BIN)
	cargo run --manifest-path ./tools/mkinitrd/Cargo.toml -- -v $(INITRD_IMG) $(INITRD_FILES)

iso: install
	mkisofs -r -J -o $(TARGET_ISO) $(MNT)

apps:
	cd apps; cargo build --target wasm32-unknown-unknown --release
	for name in ./apps/target/wasm32-unknown-unknown/release/*.wasm; do \
	cargo run --manifest-path ./tools/wasm-strip/Cargo.toml -- -preserve name -strip-all $$name $$name; done

test:
	cargo test --manifest-path lib/megstd/Cargo.toml
	cargo test --manifest-path lib/meggl/Cargo.toml
	# cargo test --manifest-path lib/wami/Cargo.toml
	cargo test --manifest-path lib/mar/Cargo.toml

doc:
	(cd system; cargo doc --all --target $(KRNL_ARCH).json)
