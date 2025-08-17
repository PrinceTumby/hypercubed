.text
.set noreorder
.align 4

.global __start
.type __start, function
__start:
    # -- Clear BSS --
    dla $t0, __bss_start
    dla $t1, __bss_end
    addi $t1, $t1, 4
    sub $t1, $t1, $t0
1:
    sw $zero, 0($t0)
    addi $t0, $t0, 4
    addi $t1, $t1, -4
    bgtz $t1, 1b
    nop

    # -- Set up main thread --
    dla $a0, _gp
    move $gp, $4
    # Place stack at end of RDRAM.
    li $a1, -1
    # 128KiB Stack
    li $a2, 128 * 1024
    # System arguments static pointer, defined in `mod.rs`.
    dla $a3, SYS_ARGS
    dla $a4, syscall_exit_thread
    li $v1, 0x3C
    syscall
    move $sp, $v0

    # -- Jump to Rust code --
    j ps2_entrypoint
    nop
.size __start, . - __start
