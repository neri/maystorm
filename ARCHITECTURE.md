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
    :load configuration from /EFI/BOOT/CONFIG.JSON;
    :find ACPI RSDPTR from EFI_CONFIGURATION_TABLE;
    :init GOP;
    :load kernel;
    note right
        default /EFI/BOOT/KERNEL.BIN
    end note
    :load initrd;
    note right
        default /EFI/BOOT/INITRD.IMG
    end note
    :invoke BootServices->ExitBootServices();
    :Initialize the page table for startup;
    :relocate Kernel;
    :start Paging;
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
    note right
        Prepare for Startup IPI
    end note
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
        note left
Since each processor that 
receives the IPI starts 
initialization asynchronously, 
the physical processor ID and 
the logical ID assigned by the 
OS are not aligned. Therefore, 
sorting is necessary here.
        end note
        :AP_STALLED ← false;
        :Cpu::set_tsc_base();
    fork again
        -[#green,dotted]->
        :received INIT & Startup IPI;
        :_smp_rm_payload;
        note right
Since the initial state is 
Real mode, it will enter 
Long mode with minimal 
initialization.
        end note
        :_ap_startup;
        note right
Minimal initialization 
for calling Rust code
        end note
        partition apic_start_ap() {
            :LocalApic::init_ap();
            :Cpu::new();
            :System::activate_cpu();
            while (AP_STALLED)
            endwhile
            :Cpu::set_tsc_base();
            note right
Synchronizes the TSC between 
each processor core.
            end note
        }
        :idle;
        note right
Now ready to schedule
        end note
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
