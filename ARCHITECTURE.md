# Architecture of MEG-OS

## Code Map

### `apps`

Applications.

### `boot`

- `/boot/boot-efi/`
  - Boot loader for UEFI

### `ext`

Files imported from outside this project.

### `lib`

Common library that spans multiple programs.

- `/lib/megstd/`
  - MEG-OS Standard Library
- `/lib/bootprot/`
  - boot protocol

### `misc`

Miscellaneous.

- `/misc/initrd/`
  - Source of initrd

### `system`

The kernel and system sources.

- `/system/kernel/src/arch/`
  - Architecture-specific code

### `tools`

Small tools used for building.

- `/tools/elf2ceef/`
  - Convert format of the kernel
- `/tools/mkfdfs/`
  - Make a floppy disk image
- `/tools/mkinitrd/`
  - Make an initrd image

## Kernel

- TBD

## Boot Sequence (UEFI)

``` plantuml
@startuml
title UEFI to Kernel
start

partition UEFI {
    :Some initialization processes;
    :load /EFI/BOOT/BOOTX64.EFI from BootDisk;
}

partition /EFI/BOOT/BOOTX64.EFI {
    :load configuration from /EFI/MEGOS/CONFIG.JSON;
    :find ACPI RSDPTR from EFI_CONFIGURATION_TABLE;
    :find SMBIOS entry point from EFI_CONFIGURATION_TABLE;
    :init GOP;
    :load kernel;
    note right
        default /EFI/MEGOS/KERNEL.BIN
    end note
    :load initrd;
    note right
        default /EFI/MEGOS/INITRD.IMG
    end note
    :invoke BootServices->ExitBootServices();
    :initialize the page table for startup;
    :relocate Kernel;
    :start paging and switch CPU mode.;
    :invoke Kernel;
}
:Kernel entry point;
stop

@enduml
```

``` plantuml
@startuml
title Kernel Initialization
start
:entry point;
:System::init();
partition System::init() {
    :MemoryManager::init();
    :make main_screen;
    :make emergency console;
    :init ACPI;
    :reserve the processor structure for the number of processors;
    :Arch::init();
    :Pci::init();
    :Scheduler::start();
}
split 
    :idle;
    detach
split again
    :System::late_init();
    partition System::late_init() {
        :Fs::init();
        :RuntimeEnvironment::init();
        :WindowManager::init();
        :HidManager::init();
        :Arch::late_init();
    }
    :UserEnv::init();
    stop
end split

@enduml
```


## Memory Manager

- MEG-OS allocates large memory blocks in pages. Smaller memory blocks are allocated with a slab allocator.

## Scheduler

- MEG-OS supports five priority-based preemptive multi-threaded schedulers.
- Priority **Real-time** is scheduled with the highest priority and is never preempted.
- The **high**, **normal**, and **low** priorities are each scheduled in a round-robin fashion and will be preempted when the allocated Quantum is consumed.
- Priority **idle** makes the processor idle when all other threads are waiting. It is never scheduled.

## Window System

- TBD

## Hid Manager

- HidManager relays between human interface devices and the window event subsystem
- Keyboard scancodes will be converted to the Usage specified by the USB-HID specification on all platforms

## FileSystem

- TBD

## User Land (Personality)

- TBD
