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
    ; pub unsafe extern "C" fn apic_handle_irq(irq: Irq)
    extern apic_handle_irq
    ; pub unsafe extern "C" fn cpu_default_exception(ctx: *mut X64StackContext)
    extern cpu_default_exception
    ; pub unsafe extern "C" fn sch_setup_new_thread()
    extern sch_setup_new_thread
    ; pub unsafe extern "C" fn cpu_int40_handler(ctx: *mut X64StackContext)
    extern cpu_int40_handler

    ; pub unsafe extern "C" fn apic_handle_irq(irq: Irq)
    extern apic_handle_irq


    ; fn asm_handle_exception(_: InterruptVector) -> usize;
    global asm_handle_exception
asm_handle_exception:
    cmp cl, 0x40
    jz .hoe
    cmp cl, 15
    ja .no_exception
    movzx ecx, cl
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

    mov rcx, rbp
    call cpu_default_exception

    lea rsp, [rbp + 8 * 5]
    ; mov rsp, rbp
    ; pop rax ; CR2
    ; pop gs
    ; pop fs
    ; pop es
    ; pop ds
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

    mov rcx, rbp
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


_irq1:
    push rax
    mov al, 1
    jmp _irq

_irq2:
    push rax
    mov al, 2
    jmp _irq

_irq3:
    push rax
    mov al, 3
    jmp _irq

_irq4:
    push rax
    mov al, 4
    jmp _irq

_irq5:
    push rax
    mov al, 5
    jmp _irq

_irq6:
    push rax
    mov al, 6
    jmp _irq

_irq7:
    push rax
    mov al, 7
    jmp _irq

_irq8:
    push rax
    mov al, 8
    jmp _irq

_irq9:
    push rax
    mov al, 9
    jmp _irq

_irq10:
    push rax
    mov al, 10
    jmp _irq

_irq11:
    push rax
    mov al, 11
    jmp _irq

_irq12:
    push rax
    mov al, 12
    jmp _irq

_irq13:
    push rax
    mov al, 13
    jmp _irq

_irq14:
    push rax
    mov al, 14
    jmp _irq

_irq15:
    push rax
    mov al, 15
    jmp _irq

_irq16:
    push rax
    mov al, 16
    jmp _irq

_irq17:
    push rax
    mov al, 17
    jmp _irq

_irq18:
    push rax
    mov al, 18
    jmp _irq

_irq19:
    push rax
    mov al, 19
    jmp _irq

_irq20:
    push rax
    mov al, 20
    jmp _irq

_irq21:
    push rax
    mov al, 21
    jmp _irq

_irq22:
    push rax
    mov al, 22
    jmp _irq

_irq23:
    push rax
    mov al, 23
    jmp _irq

_irq:
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    movzx ecx, al
    cld

    call apic_handle_irq

    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq


    ; fn asm_handle_irq_table(_table: &mut [usize; MAX_GSI], _max_gsi: usize);
    global asm_handle_irq_table
asm_handle_irq_table:
    push rsi
    push rdi
    mov rdi, rcx
    mov ecx, edx
    lea rsi, [rel _irq_table]
    lea rdx, [rel _base]
    mov eax, (_end_irq_table - _irq_table) / 4
    cmp ecx, eax
    cmova ecx, eax
.loop:
    lodsd
    or rax, rax
    jz .skip
    add rax, rdx
.skip:
    stosq
    loop .loop

    pop rdi
    pop rsi
    ret



;   fn asm_sch_switch_context(current: *mut u8, next: *mut u8);
%define CTX_USER_CS     0x10
%define CTX_USER_DS     0x18
%define CTX_SP          0x20
%define CTX_BP          0x28
%define CTX_BX          0x30
%define CTX_SI          0x38
%define CTX_DI          0x40
%define CTX_R12         0x48
%define CTX_R13         0x50
%define CTX_R14         0x58
%define CTX_R15         0x60
%define CTX_TSS_RSP0    0x68
%define CTX_DS          0x70
%define CTX_ES          0x74
%define CTX_FS          0x78
%define CTX_GS          0x7C
%define CTX_GDT_TEMP    0xF0
%define CTX_FPU_BASE    0x100
    global asm_sch_switch_context
asm_sch_switch_context:

    mov [rcx + CTX_SP], rsp
    mov [rcx + CTX_BP], rbp
    mov [rcx + CTX_BX], rbx
    mov [rcx + CTX_SI], rsi
    mov [rcx + CTX_DI], rdi
    mov [rcx + CTX_R12], r12
    mov [rcx + CTX_R13], r13
    mov [rcx + CTX_R14], r14
    mov [rcx + CTX_R15], r15
    mov [rcx + CTX_DS], ds
    mov [rcx + CTX_ES], es
    mov [rcx + CTX_FS], fs
    mov [rcx + CTX_GS], gs

    sgdt [rcx + CTX_GDT_TEMP + 6]
    mov rbx, [rcx + CTX_GDT_TEMP + 8]

    mov rax, [rdx + CTX_USER_CS]
    xchg rax, [rbx + 8 * 4]
    mov [rcx + CTX_USER_CS], rax

    mov rax, [rdx + CTX_USER_DS]
    xchg rax, [rbx + 8 * 5]
    mov [rcx + CTX_USER_DS], rax

    add rbx, 64
    mov rax, [rdx + CTX_TSS_RSP0]
    xchg rax, [rbx + TSS64_RSP0]
    mov [rcx + CTX_TSS_RSP0], rax

    mov ds, [rdx + CTX_DS]
    mov es, [rdx + CTX_ES]
    mov fs, [rdx + CTX_FS]
    mov gs, [rdx + CTX_GS]
    mov rsp, [rdx + CTX_SP]
    mov rbp, [rdx + CTX_BP]
    mov rbx, [rdx + CTX_BX]
    mov rsi, [rdx + CTX_SI]
    mov rdi, [rdx + CTX_DI]
    mov r12, [rdx + CTX_R12]
    mov r13, [rdx + CTX_R13]
    mov r14, [rdx + CTX_R14]
    mov r15, [rdx + CTX_R15]

    xor eax, eax
    xor ecx, ecx
    xor edx, edx
    xor r8d, r8d
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d

    ret


;    fn asm_sch_make_new_thread(context: *mut u8, new_sp: *mut u8, start: () -> (), args: usize,);
    global asm_sch_make_new_thread
asm_sch_make_new_thread:
    lea rax, [rel _new_thread]
    sub rdx, BYTE 0x18
    mov [rdx], rax
    mov [rdx + 0x08], r8
    mov [rdx + 0x10], r9
    mov [rcx + CTX_SP], rdx
    xor eax, eax
    mov [rcx + CTX_USER_CS], rax
    mov [rcx + CTX_USER_DS], rax
    ret


_new_thread:
    call sch_setup_new_thread
    sti
    pop rax
    pop rcx
    call rax
    ud2


;   fn asm_apic_setup_sipi(vec_sipi: u8, max_cpu: usize, stack_chunk_size: usize, stack_base: *mut u8);
    global asm_apic_setup_sipi
asm_apic_setup_sipi:
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


_irq_table:
    dd 0
    dd _irq1 - _base
    dd _irq2 - _base
    dd _irq3 - _base
    dd _irq4 - _base
    dd _irq5 - _base
    dd _irq6 - _base
    dd _irq7 - _base
    dd _irq8 - _base
    dd _irq9 - _base
    dd _irq10 - _base
    dd _irq11 - _base
    dd _irq12 - _base
    dd _irq13 - _base
    dd _irq14 - _base
    dd _irq15 - _base
    dd _irq16 - _base
    dd _irq17 - _base
    dd _irq18 - _base
    dd _irq19 - _base
    dd _irq20 - _base
    dd _irq21 - _base
    dd _irq22 - _base
    dd _irq23 - _base
_end_irq_table

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

