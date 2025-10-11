ENTRY(__start);

PROGRAM_BASE = 0x00100000;

PHDRS {
    text PT_LOAD FLAGS(0x5); /* Read and Execute */
    data PT_LOAD;
    rodata PT_LOAD FLAGS(0x4); /* Read only */
    bss PT_LOAD;
}

SECTIONS {
    . = PROGRAM_BASE;
    .text : {
        *(.text*)
    } :text
    .data ALIGN(4K) : {
        *(.data*)
    } :data
    .rodata ALIGN(4K) : {
        *(.rodata*)
    } :rodata
    .bss ALIGN(4K) : {
        *(.bss*)
        *(COMMON)
    } :bss
    PROVIDE(HEAP_START = .);

    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
    }
}
