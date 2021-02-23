# Architecture of MYOS

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
    :load config from /EFI/BOOT/CONFIG.JSON;
    :find ACPI RSDPTR;
    :init GOP;
    :load Kernel from /EFI/BOOT/kernel.bin;
    :load initrd from /EFI/BOOT/initrd.img;
    :invoke BootServices->ExitBootServices();
    :Initialize the page table for startup;
    :relocate Kernel;
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
    :make main_screen;
    :MemoryManager::init();
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

``` plantuml
@startuml
title Arch::init (x64)
start
:Cpu::init();
partition Apic::init() {
    :Some initialization processes;
    :asm_apic_setup_sipi();
    :LocalApic::broadcast_init();
    :LocalApic::broadcast_startup();
    fork
        while (are all APs active?)
            if (timed out?) then (yes)
                :panic;
                end
            endif
        endwhile
        :System::sort_cpus();
        :AP_STALLED ← false;
        :Cpu::set_tsc_base();
    fork again
        :_smp_rm_payload (RealMode);
        :_ap_startup (LongMode);
        partition apic_start_ap() {
            :LocalApic::init_ap();
            :Cpu::new();
            :System::activate_cpu();
            while (AP_STALLED)
            endwhile
            :Cpu::set_tsc_base();
        }
        :idle;
        detach
    end fork
}
:Some initialization processes;
stop
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
