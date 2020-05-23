;;

%define KERNEL_CS64         0x10
%define KERNEL_SS           0x18

%define IA32_MISC           0x000001A0
%define IA32_EFER           0xC0000080

%define SMPINFO             0x0800
%define SMPINFO_MAX_CPU     0x04
%define SMPINFO_EFER        0x08
%define SMPINFO_STACK_SIZE  0x0C
%define SMPINFO_STACK_BASE  0x10
%define SMPINFO_CR3         0x18
%define SMPINFO_IDT         0x22
%define SMPINFO_CR4         0x2C
%define SMPINFO_START64     0x30
%define SMPINFO_AP_STARTUP  0x38
%define SMPINFO_MSR_MISC    0x40
%define SMPINFO_GDTR        0x50

[bits 64]
[section .text]

;   pub unsafe extern "efiapi" fn apic_handle_irq(irq: Irq);
    extern apic_handle_irq

_irq2:
    push rcx
    mov cl, 2
    jmp _irqXX
_irq1:
    push rcx
    mov cl, 1
    jmp _irqXX
_irq0:
    push rcx
    mov cl, 0
;   jmp _irqXX

_irqXX:
    push rax
    push rdx
    push r8
    push r9
    push r10
    push r11
    cld

    call apic_handle_irq

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdx
    pop rax
    pop rcx
    iretq

    extern apic_start_ap

;   fn setup_smp_init(vec_sipi: u8, max_cpu: usize, stack_chunk_size: usize, stack_base: *mut u8);
    global setup_smp_init
setup_smp_init:
    push rsi
    push rdi

    movzx r11d, cl
    shl r11d, 12
    mov edi, r11d
    lea rsi, [rel _smp_rm_payload]
    mov ecx, _end_smp_rm_payload - _smp_rm_payload
    rep movsb

    mov r10d, SMPINFO
    mov [r10 + SMPINFO_MAX_CPU], edx
    mov [r10 + SMPINFO_STACK_SIZE], r8d
    mov [r10 + SMPINFO_STACK_BASE], r9
    lea edx, [r10 + SMPINFO_GDTR]
    lea rsi, [rel _minimal_GDT]
    mov edi, edx
    mov ecx, (_end_GDT - _minimal_GDT)/4
    rep movsd
    mov [edx+2], edx
    mov word [edx], (_end_GDT - _minimal_GDT)-1

    mov edx, 1
    mov [r10], edx
    mov rdx, cr4
    mov [r10 + SMPINFO_CR4], edx
    mov rdx, cr3
    mov [r10 + SMPINFO_CR3], rdx
    sidt [r10 + SMPINFO_IDT]
    mov ecx, IA32_EFER
    rdmsr
    mov [r10 + SMPINFO_EFER], eax
    mov ecx, IA32_MISC
    rdmsr
    mov [r10 + IA32_MISC], eax
    mov [r10 + IA32_MISC + 4], edx

    lea ecx, [r11 + _startup64 - _smp_rm_payload]
    mov edx, KERNEL_CS64
    mov [r10 + SMPINFO_START64], ecx
    mov [r10 + SMPINFO_START64 + 4], edx
    lea rax, [rel _ap_startup]
    mov [r10 + SMPINFO_AP_STARTUP], rax

    mov eax, r10d
    pop rdi
    pop rsi
    ret


_ap_startup:
    lidt [rbx + SMPINFO_IDT]

    ; init stack pointer
    mov eax, ebp
    imul eax, [rbx + SMPINFO_STACK_SIZE]
    mov rcx, [rbx + SMPINFO_STACK_BASE]
    lea rsp, [rcx + rax]

    ; init APIC
    mov ecx, ebp
    call apic_start_ap

    ; idle thread
    sti
.loop:
    hlt
    jmp .loop


    ; Payload SMP initialization
[bits 16]
_smp_rm_payload:
    cli
    xor ax, ax
    mov ds, ax
    mov ebx, SMPINFO

    ; acquire core-id
    mov al, [bx]
    mov cl, [bx + SMPINFO_MAX_CPU]
.loop:
    cmp al, cl
    jae .fail
    mov dl, al
    inc dx
    lock cmpxchg [bx], dl
    jz .core_ok
    pause
    jmp short .loop
.fail:
.forever:
    hlt
    jmp short .forever

.core_ok:
    movzx ebp, al

    lgdt [bx + SMPINFO_GDTR]

    ; enter to PM
    mov eax, cr0
    bts eax, 0
    mov cr0, eax

    mov ax, KERNEL_SS
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; restore BSP's system registers
    mov eax, [bx + SMPINFO_CR4]
    mov cr4, eax
    mov eax, [bx + SMPINFO_CR3]
    mov cr3 ,eax

    mov eax, [bx + SMPINFO_MSR_MISC]
    mov edx, [bx + SMPINFO_MSR_MISC + 4]
    mov ecx, IA32_MISC
    wrmsr

    mov ecx, IA32_EFER
    xor edx, edx
    mov eax, [bx+ SMPINFO_EFER]
    wrmsr

    ; enter to LM
    mov eax, cr0
    bts eax, 31
    mov cr0, eax

    ; o32 jmp far [bx + SMPINFO_START64]
    jmp far dword [bx + SMPINFO_START64]

[BITS 64]

_startup64:
    jmp [rbx + SMPINFO_AP_STARTUP]

_end_smp_rm_payload:

    ; Boot time minimal GDT
_minimal_GDT:
    dw 0, 0, 0, 0                       ; 00 NULL
    dw 0xFFFF, 0x0000, 0x9A00, 0x00CF   ; 08 DPL0 CODE32 FLAT HISTORICAL
    dw 0xFFFF, 0x0000, 0x9A00, 0x00AF   ; 10 DPL0 CODE64 FLAT
    dw 0xFFFF, 0x0000, 0x9200, 0x00CF   ; 18 DPL0 DATA FLAT MANDATORY
_end_GDT:
