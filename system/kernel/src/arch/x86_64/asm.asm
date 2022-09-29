;;

[bits 64]
[section .text]
_base:

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
    cmp dil, 0x13
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

_asm_int_07: ; #NM Device Not Available
    push BYTE 0
    push BYTE 0x07
    jmp short _exception

_asm_int_08: ; #DF Double Fault
    push BYTE 0x08
    jmp short _exception

_asm_int_0D: ; #GP General Protection Fault
    push BYTE 0x0D
    jmp short _exception

_asm_int_0E: ; #PF Page Fault
    push BYTE 0x0E
    jmp short _exception

_asm_int_13: ; #XM SIMD Floating-Point Exception
    push BYTE 0
    push BYTE 0x13
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
    xor eax, eax
    push rax
    stmxcsr [rsp]
    mov rbp, rsp
    and rsp, byte 0xF0
    ; sub rsp, 0x100
    ; lea rax, [rsp + 0x80]
    ; movaps [rax + 0x70], xmm15
    ; movaps [rax + 0x60], xmm14
    ; movaps [rax + 0x50], xmm13
    ; movaps [rax + 0x40], xmm12
    ; movaps [rax + 0x30], xmm11
    ; movaps [rax + 0x20], xmm10
    ; movaps [rax + 0x10], xmm9
    ; movaps [rax], xmm8
    ; movaps [rax - 0x10], xmm7
    ; movaps [rax - 0x20], xmm6
    ; movaps [rax - 0x30], xmm5
    ; movaps [rax - 0x40], xmm4
    ; movaps [rax - 0x50], xmm3
    ; movaps [rax - 0x60], xmm2
    ; movaps [rax - 0x70], xmm1
    ; movaps [rax - 0x80], xmm0
    cld

    mov rdi, rbp
    call cpu_default_exception

    ; lea rax, [rbp - 0x80]
    ; movaps xmm0, [rax - 0x80]
    ; movaps xmm1, [rax - 0x70]
    ; movaps xmm2, [rax - 0x60]
    ; movaps xmm3, [rax - 0x50]
    ; movaps xmm4, [rax - 0x40]
    ; movaps xmm5, [rax - 0x30]
    ; movaps xmm6, [rax - 0x20]
    ; movaps xmm7, [rax - 0x10]
    ; movaps xmm8, [rax]
    ; movaps xmm9, [rax + 0x10]
    ; movaps xmm10, [rax + 0x20]
    ; movaps xmm11, [rax + 0x30]
    ; movaps xmm12, [rax + 0x40]
    ; movaps xmm13, [rax + 0x50]
    ; movaps xmm14, [rax + 0x60]
    ; movaps xmm15, [rax + 0x70]
    lea rsp, [rbp + 8 * 6]
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
    sub rsp, byte 24
    mov rbp, rsp
    mov [rbp], eax
    mov [rbp + 4], ecx
    mov [rbp + 8], edx
    mov [rbp + 12], ebx
    mov [rbp + 16], esi
    mov [rbp + 20], edi
    mov eax, [rbp + 32]
    mov [rbp + 28], eax
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
    mov r8d, [rbp + 24]
    lea rsp, [rbp + 8 * 4]
    mov ebp, r8d
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
    fxsave [rdi + CTX_FPU_BASE]

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

    fxrstor [rsi + CTX_FPU_BASE]
    mov rsp, [rsi + CTX_SP]
    mov ds, [rsi + CTX_DS]
    mov es, [rsi + CTX_ES]
    mov fs, [rsi + CTX_FS]
    mov gs, [rsi + CTX_GS]
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


;    fn asm_sch_get_context_status(context: *const u8, result: *mut &[usize; 2]);
    global asm_sch_get_context_status
asm_sch_get_context_status:
    mov rax, [rdi + CTX_SP]
    mov [rsi], rax
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
    fninit
    ldmxcsr [rel _mxcsr]
    pxor xmm0, xmm0
    pxor xmm1, xmm1
    pxor xmm2, xmm2
    pxor xmm3, xmm3
    pxor xmm4, xmm4
    pxor xmm5, xmm5
    pxor xmm6, xmm6
    pxor xmm7, xmm7
    pxor xmm8, xmm8
    pxor xmm9, xmm9
    pxor xmm10, xmm10
    pxor xmm11, xmm11
    pxor xmm12, xmm12
    pxor xmm13, xmm13
    pxor xmm14, xmm14
    pxor xmm15, xmm15

    call sch_setup_new_thread
    sti
    pop rax
    pop rdi
    call rax
    ud2


[section .rodata]

_mxcsr:
    dd 0x00001F80

_exception_table:
    dd _asm_int_00 - _base
    dd 0 ; int_01
    dd 0 ; int_02
    dd _asm_int_03 - _base
    dd 0 ; int_04
    dd 0 ; int_05
    dd _asm_int_06 - _base
    dd _asm_int_07 - _base
    dd _asm_int_08 - _base
    dd 0 ; int_09
    dd 0 ; int_0A
    dd 0 ; int_0B
    dd 0 ; int_0C
    dd _asm_int_0D - _base
    dd _asm_int_0E - _base
    dd 0 ; int_0F
    dd 0 ; int_10
    dd 0 ; int_11
    dd 0 ; int_12
    dd _asm_int_13 - _base

