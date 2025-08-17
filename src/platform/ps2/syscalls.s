.text
.set noreorder

.macro defineSyscall name number
.set push
.set noreorder
.text
.global \name
.type \name, @function
.ent \name, 0
\name:
    j __syscall
    li $v1, \number
.end \name
.size \name, . - \name
.set pop
.endm

.set push
.set noreorder
.text
.align 4
.global __syscall
.type __syscall, @function
.ent __syscall, 0
__syscall:
    mfc0 $2, $12
    andi $2, $2, 0x18
    beqz $2, _kMode
    slt $2, $3, $0
    syscall
    jr $31
    nop
_kMode:
    subu $26, $0, $3
    movn $3, $26, $2
    sll $3, $3, 2
    lui $26, 0x8000
    lhu $2, 0x02F0($26)
    sll $2, $2, 16
    lh $26, 0x02F8($26)
    add $2, $26
    addu $3, $2
    lw $26, 0x00($3)
    jr $26
    nop
.end	__syscall
.size	__syscall, . - __syscall
.set	pop

defineSyscall syscall_gs_set_crt 0x02
defineSyscall syscall_add_intc_handler 0x10
defineSyscall syscall_remove_intc_handler 0x11
defineSyscall syscall_add_dmac_handler 0x12
defineSyscall syscall_remove_dmac_handler 0x13
defineSyscall syscall_enable_dmac 0x16
defineSyscall syscall_disable_dmac 0x17
defineSyscall syscall_exit_thread 0x23
defineSyscall syscall_init_heap 0x3D
defineSyscall syscall_create_sema 0x40
defineSyscall syscall_delete_sema 0x41
defineSyscall syscall_signal_sema 0x42
defineSyscall syscall_wait_sema 0x44
defineSyscall syscall_gs_get_imr 0x70
defineSyscall syscall_gs_set_imr 0x71
defineSyscall syscall_pcsx2_printf 0x75
defineSyscall syscall_i_enable_dmac, -0x1C
defineSyscall syscall_i_disable_dmac, -0x1D
