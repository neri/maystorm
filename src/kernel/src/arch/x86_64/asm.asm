;;

%define KERNEL_CS64         0x10
%define KERNEL_SS           0x18

%define IA32_MISC           0x000001A0
%define IA32_EFER           0xC0000080

%define CR0_PE              0
%define CR0_PG              31
%define CR4_PAE             5
%define EFER_LME            8
%define EFER_LMA            10

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

    global _int00
    global _int03
    global _int06
    global _int08
    global _int0D
    global _int0E

_int00: ; #DE
    push BYTE 0
    push BYTE 0x00
    jmp short _intXX

_int03: ; #BP
    push BYTE 0
    push BYTE 0x03
    jmp short _intXX

_int06: ; #UD
    push BYTE 0
    push BYTE 0x06
    jmp short _intXX

_int08: ; #DF
    push BYTE 0x08
    jmp short _intXX

_int0D: ; #GP
    push BYTE 0x0D
    jmp short _intXX

_int0E: ; #PF
    push BYTE 0x0E
    ; jmp short _intXX

    extern default_int_ex_handler
_intXX:
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
    mov rax, cr2
    push rax
    cld

    mov rcx, rsp
    call default_int_ex_handler

    pop rax ; CR2
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
    add rsp, BYTE 16 ; err/intnum
_iretq:
    iretq


;   fn sch_switch_context(current: *mut u8, next: *mut u8);
%define CTX_SP          0x08
%define CTX_BP          0x10
%define CTX_BX          0x18
%define CTX_SI          0x20
%define CTX_DI          0x28
%define CTX_R12         0x30
%define CTX_R13         0x38
%define CTX_R14         0x40
%define CTX_R15         0x48
%define CTX_TSS_RSP0    0x50
%define CTX_FPU_BASE    0x80
    global sch_switch_context
sch_switch_context:

    mov [rcx + CTX_SP], rsp
    mov [rcx + CTX_BP], rbp
    mov [rcx + CTX_BX], rbx
    mov [rcx + CTX_SI], rsi
    mov [rcx + CTX_DI], rdi
    mov [rcx + CTX_R12], r12
    mov [rcx + CTX_R13], r13
    mov [rcx + CTX_R14], r14
    mov [rcx + CTX_R15], r15

    ; call cpu_get_tss
    ; mov r11, [rax + TSS64_RSP0]
    ; mov r10, [rdx + CTX_TSS_RSP0]
    ; mov [rcx + CTX_TSS_RSP0], r11
    ; mov [rax + TSS64_RSP0], r10

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


;    fn sch_make_new_thread(context: *mut u8, new_sp: *mut u8, start: *mut c_void, args: *mut c_void,);
    global sch_make_new_thread
sch_make_new_thread:
    lea rax, [rel _new_thread]
    sub rdx, BYTE 0x18
    mov [rdx], rax
    mov [rdx + 0x08], r8
    mov [rdx + 0x10], r9
    mov [rcx + CTX_SP], rdx
    ret


    extern sch_setup_new_thread
_new_thread:
    call sch_setup_new_thread
    sti
    pop rax
    pop rcx
    call rax
    ud2




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


    ; SMP initialization payload
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
