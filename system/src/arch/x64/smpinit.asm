;; SMP init module

%define KERNEL_CS64         0x08
%define KERNEL_SS           0x10

%define IA32_MISC           0x000001A0
%define IA32_EFER           0xC0000080

%define CR0_PE              0
%define CR0_TS              3
%define CR0_PG              31
%define CR4_PAE             5
%define EFER_LME            8
%define EFER_LMA            10
%define EFER_NXE            11
%define MISC_XD_DISABLE     2

%define SMPINFO             0x0800
%define SMPINFO_MAX_CPU     0x04
%define SMPINFO_EFER        0x08
;define SMPINFO_            0x0C
%define SMPINFO_STACK_BASE  0x10
%define SMPINFO_CR3         0x18
%define SMPINFO_CR4         0x20
%define SMPINFO_IDT         0x26
%define SMPINFO_START64     0x30
%define SMPINFO_AP_STARTUP  0x38
%define SMPINFO_MSR_MISC    0x40
%define SMPINFO_GDTR        0x50

[section .text]
_begin_payload:

[bits 16]
_sipi_handler:
    jmp short _now_in_rm

    alignb 8
[bits 64]
    jmp _prepare_sipi

[bits 16]
    alignb 16
_now_in_rm:
    cli
    xor ax, ax
    mov ds, ax
    mov ebp, SMPINFO

    ; acquire a temporary core-id
    mov ax, 1
    lock xadd [bp], ax
    cmp ax, [bp + SMPINFO_MAX_CPU]
    jae .fail
    jmp .core_ok
.fail:
.forever:
    hlt
    jmp short .forever

.core_ok:
    movzx edi, ax

    lgdt [bp + SMPINFO_GDTR]

    ; enter to minimal PM
    mov eax, cr0
    bts eax, CR0_PE
    mov cr0, eax

    mov ax, KERNEL_SS
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    xor eax, eax
    cpuid
    cmp ebx, 0x756e6547
    jnz .nointel
    cmp edx, 0x49656e69
    jnz .nointel
    cmp ecx, 0x6c65746e
    jnz .nointel
    mov ecx, IA32_MISC
    rdmsr
    btr edx, MISC_XD_DISABLE
    wrmsr
.nointel:

    ; restore BSP's system registers
    mov eax, [bp + SMPINFO_CR4]
    mov cr4, eax

    mov eax, [bp + SMPINFO_CR3]
    mov cr3 ,eax

    mov ecx, IA32_EFER
    xor edx, edx
    mov eax, [bp+ SMPINFO_EFER]
    wrmsr

    ; enter to LM
    mov eax, cr0
    bts eax, CR0_PG
    mov cr0, eax

    jmp far dword [bp + SMPINFO_START64]


[BITS 64]
_now_in_64bit_mode:
    lidt [rbp + SMPINFO_IDT]

    ; init stack pointer
    mov rax, [rbp + SMPINFO_STACK_BASE]
    mov rsp, [rax + rdi * 8]

    ; jump to kernel
    call [rbp + SMPINFO_AP_STARTUP]
    ud2


;   extern "C" fn(max_cpu: usize, stacks: *const *mut u8, start_ap: unsafe extern "C" fn());
_prepare_sipi:
    mov r8, rsi
    mov r9, rdx
    mov edx, edi

    mov r10d, SMPINFO
    mov [r10 + SMPINFO_MAX_CPU], edx
    mov [r10 + SMPINFO_STACK_BASE], r8
    lea edx, [r10 + SMPINFO_GDTR]
    lea rsi, [rel _minimal_GDT]
    lea edi, [rdx + 8]
    mov ecx, (_end_GDT - _minimal_GDT) / 4
    rep movsd
    mov [rdx + 2], edx
    mov word [rdx], (_end_GDT - _minimal_GDT) + 7

    mov ecx, 1
    mov [r10], ecx
    mov rdx, cr4
    mov [r10 + SMPINFO_CR4], edx
    mov rdx, cr3
    mov [r10 + SMPINFO_CR3], rdx
    sidt [r10 + SMPINFO_IDT]
    mov ecx, IA32_EFER
    rdmsr
    btr eax, EFER_LMA
    mov [r10 + SMPINFO_EFER], eax

    lea ecx, [rel _now_in_64bit_mode]
    mov [r10 + SMPINFO_START64], ecx
    mov dword [r10 + SMPINFO_START64 + 4], KERNEL_CS64
    mov [r10 + SMPINFO_AP_STARTUP], r9

    ret


[section .rodata]

    alignb 16

    ; Boot time minimal GDT
_minimal_GDT:
    dw 0xFFFF, 0x0000, 0x9A00, 0x00AF   ; 08 DPL0 CODE64 FLAT
    dw 0xFFFF, 0x0000, 0x9200, 0x00CF   ; 10 DPL0 DATA FLAT MANDATORY
_end_GDT:

_end_payload:
