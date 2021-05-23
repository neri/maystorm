;;

%define KERNEL_CS64         0x08
%define KERNEL_SS           0x10

%define IA32_MISC           0x000001A0
%define IA32_EFER           0xC0000080

%define CR0_PE              0
%define CR0_PG              31
%define CR4_PAE             5
%define EFER_LME            8
%define EFER_LMA            10

%define TSS64_RSP0          0x04

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
_base:

    ; pub unsafe extern "C" fn apic_start_ap(_cpuid: u8)
    extern apic_start_ap
    ; pub unsafe extern "C" fn cpu_default_exception(ctx: *mut X64StackContext)
    extern cpu_default_exception
    ; pub unsafe extern "C" fn sch_setup_new_thread()
    extern sch_setup_new_thread
    ; pub unsafe extern "C" fn cpu_int40_handler(ctx: *mut X64StackContext)
    extern cpu_int40_handler


    ; fn asm_handle_exception(_: InterruptVector) -> usize;
    global asm_handle_exception
asm_handle_exception:
    cmp dil, 0x40
    jz .hoe
    cmp dil, 15
    ja .no_exception
    movzx ecx, dil
    lea rdx, [rel _exception_table]
    mov eax, [rdx + rcx * 4]
    or eax, eax
    jz .no_exception
    lea rdx, [rel _base]
    add rax, rdx
    ret
.no_exception:
    xor eax, eax
    ret
.hoe:
    lea rax, [rel _asm_int_40]
    ret


_asm_int_00: ; #DE Divide Error
    push BYTE 0
    push BYTE 0x00
    jmp short _exception

_asm_int_03: ; #BP Breakpoint
    push BYTE 0
    push BYTE 0x03
    jmp short _exception

_asm_int_06: ; #UD Invalid Opcode
    push BYTE 0
    push BYTE 0x06
    jmp short _exception

_asm_int_08: ; #DF Double Fault
    push BYTE 0x08
    jmp short _exception

_asm_int_0D: ; #GP General Protection Fault
    push BYTE 0x0D
    jmp short _exception

_asm_int_0E: ; #PF Page Fault
    push BYTE 0x0E
    ; jmp short _exception

_exception:
    push rax
    push rcx
    push rdx
    push rbx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov eax, ds
    push rax
    mov ecx, es
    push rcx
    push fs
    push gs
    mov rax, cr2
    push rax
    mov rbp, rsp
    and rsp, byte 0xF0
    cld

    mov rdi, rbp
    call cpu_default_exception

    lea rsp, [rbp + 8 * 5]
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rbx
    pop rdx
    pop rcx
    pop rax
    add rsp, byte 16 ; err/intnum
_iretq:
    iretq


_asm_int_40: ; INT40 Haribote OS SVC
    push rbp
    push rsi
    push rdx
    push rax
    mov rbp, rsp
    mov [rbp + 4], ecx
    mov [rbp + 12], ebx
    mov [rbp + 20], edi
    and rsp, byte 0xF0
    cld

    mov rdi, rbp
    call cpu_int40_handler

    mov eax, [rbp]
    mov ecx, [rbp + 4]
    mov edx, [rbp + 8]
    mov ebx, [rbp + 12]
    mov esi, [rbp + 16]
    mov edi, [rbp + 20]
    mov r8, [rbp + 24]
    lea rsp, [rbp + 8 * 4]
    mov rbp, r8
    iretq




;   fn asm_sch_switch_context(current: *mut u8, next: *mut u8);
%define CTX_USER_CS     0x10
%define CTX_USER_DS     0x18
%define CTX_SP          0x20
%define CTX_BP          0x28
%define CTX_BX          0x30
%define CTX_R12         0x38
%define CTX_R13         0x40
%define CTX_R14         0x48
%define CTX_R15         0x50
%define CTX_TSS_RSP0    0x58
%define CTX_DS          0x60
%define CTX_ES          0x64
%define CTX_FS          0x68
%define CTX_GS          0x6C
%define CTX_GDT_TEMP    0xF0
%define CTX_FPU_BASE    0x100
    global asm_sch_switch_context
asm_sch_switch_context:

    mov [rdi + CTX_SP], rsp
    mov [rdi + CTX_BP], rbp
    mov [rdi + CTX_BX], rbx
    mov [rdi + CTX_R12], r12
    mov [rdi + CTX_R13], r13
    mov [rdi + CTX_R14], r14
    mov [rdi + CTX_R15], r15
    mov [rdi + CTX_DS], ds
    mov [rdi + CTX_ES], es
    mov [rdi + CTX_FS], fs
    mov [rdi + CTX_GS], gs

    sgdt [rdi + CTX_GDT_TEMP + 6]
    mov rbx, [rdi + CTX_GDT_TEMP + 8]

    mov rax, [rsi + CTX_USER_CS]
    xchg rax, [rbx + 8 * 4]
    mov [rdi + CTX_USER_CS], rax

    mov rax, [rsi + CTX_USER_DS]
    xchg rax, [rbx + 8 * 5]
    mov [rdi + CTX_USER_DS], rax

    add rbx, 64
    mov rax, [rsi + CTX_TSS_RSP0]
    xchg rax, [rbx + TSS64_RSP0]
    mov [rdi + CTX_TSS_RSP0], rax

    mov ds, [rsi + CTX_DS]
    mov es, [rsi + CTX_ES]
    mov fs, [rsi + CTX_FS]
    mov gs, [rsi + CTX_GS]
    mov rsp, [rsi + CTX_SP]
    mov rbp, [rsi + CTX_BP]
    mov rbx, [rsi + CTX_BX]
    mov r12, [rsi + CTX_R12]
    mov r13, [rsi + CTX_R13]
    mov r14, [rsi + CTX_R14]
    mov r15, [rsi + CTX_R15]

    xor eax, eax
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor edi, edi
    xor r8, r8
    xor r9, r9
    xor r10, r10
    xor r11, r11

    ret


;    fn asm_sch_make_new_thread(context: *mut u8, new_sp: *mut u8, start: () -> (), args: usize,);
    global asm_sch_make_new_thread
asm_sch_make_new_thread:
    lea rax, [rel _new_thread]
    sub rsi, BYTE 0x18
    mov [rsi], rax
    mov [rsi + 0x08], rdx
    mov [rsi + 0x10], rcx
    mov [rdi + CTX_SP], rsi
    xor eax, eax
    mov [rdi + CTX_USER_CS], rax
    mov [rdi + CTX_USER_DS], rax
    ret


_new_thread:
    call sch_setup_new_thread
    sti
    pop rax
    pop rdi
    call rax
    ud2


;   fn asm_apic_setup_sipi(vec_sipi: u8, max_cpu: usize, stack_chunk_size: usize, stack_base: *mut u8);
    global asm_apic_setup_sipi
asm_apic_setup_sipi:
    mov r8, rdx
    mov r9, rcx
    mov edx, esi

    movzx r11d, dil
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
    ; mov ecx, IA32_MISC
    ; rdmsr
    ; mov [r10 + IA32_MISC], eax
    ; mov [r10 + IA32_MISC + 4], edx

    lea ecx, [r11 + _startup64 - _smp_rm_payload]
    mov edx, KERNEL_CS64
    mov [r10 + SMPINFO_START64], ecx
    mov [r10 + SMPINFO_START64 + 4], edx
    lea rax, [rel _ap_startup]
    mov [r10 + SMPINFO_AP_STARTUP], rax

    mov eax, r10d
    ret


_ap_startup:
    lidt [rbx + SMPINFO_IDT]

    ; init stack pointer
    mov eax, ebp
    imul eax, [rbx + SMPINFO_STACK_SIZE]
    mov rcx, [rbx + SMPINFO_STACK_BASE]
    lea rsp, [rcx + rax]

    ; init APIC
    mov edi, ebp
    call apic_start_ap

    ; idle thread
    sti
.loop:
    hlt
    jmp .loop




[section .rodata]
    ; Boot time minimal GDT
_minimal_GDT:
    dw 0xFFFF, 0x0000, 0x9A00, 0x00AF   ; 08 DPL0 CODE64 FLAT
    dw 0xFFFF, 0x0000, 0x9200, 0x00CF   ; 10 DPL0 DATA FLAT MANDATORY
_end_GDT:

_exception_table:
    dd _asm_int_00 - _base
    dd 0 ; int_01
    dd 0 ; int_02
    dd _asm_int_03 - _base
    dd 0 ; int_04
    dd 0 ; int_05
    dd _asm_int_06 - _base
    dd 0 ; int_07
    dd _asm_int_08 - _base
    dd 0 ; int_09
    dd 0 ; int_0A
    dd 0 ; int_0B
    dd 0 ; int_0C
    dd _asm_int_0D - _base
    dd _asm_int_0E - _base
    dd 0 ; int_0F




    ; SMP initialization payload
[bits 16]
_smp_rm_payload:
    cli
    xor ax, ax
    mov ds, ax
    mov ebx, SMPINFO

    ; acquire a temporary core-id
    mov ax, 1
    lock xadd [bx], ax
    cmp ax, [bx + SMPINFO_MAX_CPU]
    jae .fail
    jmp .core_ok
.fail:
.forever:
    hlt
    jmp short .forever

.core_ok:
    movzx ebp, ax

    lgdt [bx + SMPINFO_GDTR]

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

    ; restore BSP's system registers
    mov eax, [bx + SMPINFO_CR4]
    mov cr4, eax
    mov eax, [bx + SMPINFO_CR3]
    mov cr3 ,eax

    ; mov eax, [bx + SMPINFO_MSR_MISC]
    ; mov edx, [bx + SMPINFO_MSR_MISC + 4]
    ; mov ecx, IA32_MISC
    ; wrmsr

    mov ecx, IA32_EFER
    xor edx, edx
    mov eax, [bx+ SMPINFO_EFER]
    wrmsr

    ; enter to LM
    mov eax, cr0
    bts eax, CR0_PG
    mov cr0, eax

    jmp far dword [bx + SMPINFO_START64]

[BITS 64]
    ;; This is a buffer zone for jumping from a 16-bit segment to a 4GB
    ;; or larger address in a 64-bit segment.
_startup64:
    jmp [rbx + SMPINFO_AP_STARTUP]

_end_smp_rm_payload:

