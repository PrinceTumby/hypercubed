.text
.set noreorder

.macro startFunc name
.set push
.set noreorder
.text
.align 4
.global \name
.type \name, @function
\name:
.endm

.macro endFunc name
.size \name, . - \name
.set pop
.endm

# Memory Reading and Writing

startFunc write_u64
    dsll32 $t0, $a2, 0
    or $t0, $t0, $a1
    jr $ra
    sd $t0, ($a0)
endFunc write_u64

# Synchronisation

startFunc sync_p
    sync.p
    nop
    jr $ra
    nop
endFunc sync_p

.p2align 3
startFunc _sync_d_cache
    lui $7, 0xffff
    daddu $6, $0, $0
    ori $7, 0xf000
    nop
1:
    sync
    cache 0x10, 0($6)
    sync
    mfc0 $2, $28
    and $2, $7
    addu $2, $6
    sltu $3, $5, $2
    sltu $2, $4
    bnez $2, 2f
    nop
    bnez $3, 2f
    nop
    sync
    # 0x14: Index, Writeback, Invalidate
    cache 0x14, 0($6)
    sync
2:
    sync
    cache 0x10, 1($6)
    sync
    mfc0 $2, $28
    and $2, $7
    addu $2, $6
    sltu $3, $5, $2
    sltu $2, $4
    bnez $2, 3f
    nop
    bnez $3, 3f
    nop
    sync
    # Ditto
    cache 0x14, 1($6)
    sync
3:
    sync
    addiu $6, 64
    slti $2, $6, 4096
    bnez $2, 1b
    nop
    jr $31
    nop
endFunc _sync_d_cache

# Interrupts

startFunc disable_interrupts
    mfc0 $v0, $12
    srl $v0, $v0, 16
    andi $v0, $v0, 1
    beqz $v0, 1f
    nop
    .p2align 3
# Main loop
0:
    di
    sync.p
    mfc0 $t0, $12
    srl $t0, $t0, 16
    andi $t0, $t0, 1
    bne $t0, $zero, 0b
    nop
1:
    jr $ra
    nop
endFunc disable_interrupts

startFunc enable_interrupts
    mfc0 $v0, $12
    ei
    srl $v0, $v0, 16
    jr $ra
    andi $v0, $v0, 1
endFunc enable_interrupts

# DMA

startFunc dma_wait_fast
    sync.l
    sync.p
0:
    bc0t 1f
    nop
    bc0t 1f
    nop
    bc0t 1f
    nop
    bc0f 0b
    nop
1:
    jr $ra
    nop
endFunc dma_wait_fast
