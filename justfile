set shell := ["bash", "-uc"]
set windows-shell := ["cmd.exe", "/c"]

set ignore-comments

os := os()
os_family := os_family()

# Command replacements
wsl := if os_family == "windows" { "wsl -d Ubuntu" } else { "" }
wsl24 := if os_family == "windows" { "wsl -d Ubuntu-24.04" } else { "" }
rm := if os_family == "windows" { "del" } else { "rm" }
rmdir := if os_family == "windows" { "rmdir /s /q" } else { "rm -r" }
copy := if os_family == "windows" { "1>NUL copy /b /y" } else { "cp" }
copydir := if os_family == "windows" { "1>NUL xcopy /c /q /e /i" } else { "cp -r" }
mkdir_create_parents := if os_family == "windows" { "mkdir" } else { "mkdir -p" }
silence_stderr := if os_family == "windows" { "2>NUL" } else { "2>/dev/null" }
ignore_error := if os_family == "windows" { "|| rem" } else { "|| true" }

@build-with-vk-shaders *BUILD_FLAGS:
    echo - Compiling Vulkan shaders...
    cd crates/minecraft_client_vk_shaders/builder && cargo build
    echo - Compiling client...
    cargo build {{BUILD_FLAGS}}


# Playstation 2

_ps2_out_dir := join("target", "mipsel-sony-ps2", "dev-ps2")
_ps2_unstripped_bin := join(_ps2_out_dir, "minecraft_client_unstripped")
_ps2_elf := join(_ps2_out_dir, "minecraft_client.elf")

@check-ps2:
    cargo check \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache

@clippy-ps2:
    cargo clippy \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache

@build-ps2:
    echo - Compiling...
    cargo build \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache \
        --profile=dev-ps2 \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem,no-f16-f128,compiler-builtins-no-f16-f128
    {{wsl}} \
        /usr/local/ps2dev/ee/bin/mips64r5900el-ps2-elf-gcc \
        src/platform/ps2/entry.s \
        src/platform/ps2/syscalls.s \
        src/platform/ps2/asm_helpers.s \
        -Wl,--gc-sections \
        target/mipsel-sony-ps2/dev-ps2/libminecraft_client.a \
        -mabi=64 \
        -Wl,--no-warn-mismatch \
        -T targets/ps2.ld \
        -o target/mipsel-sony-ps2/dev-ps2/minecraft_client.elf \
        -ffreestanding \
        -nostdlib \
        -lgcc \
        -O2

@build-release-ps2:
    echo - Compiling...
    cargo build \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache \
        --profile=release-ps2 \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem,no-f16-f128,compiler-builtins-no-f16-f128
    {{wsl}} \
        /usr/local/ps2dev/ee/bin/mips64r5900el-ps2-elf-gcc \
        src/platform/ps2/entry.s \
        src/platform/ps2/syscalls.s \
        src/platform/ps2/asm_helpers.s \
        -Wl,--gc-sections \
        target/mipsel-sony-ps2/release-ps2/libminecraft_client.a \
        -mabi=64 \
        -Wl,--no-warn-mismatch \
        -T targets/ps2.ld \
        -o target/mipsel-sony-ps2/release-ps2/minecraft_client.elf \
        -ffreestanding \
        -nostdlib \
        -lgcc \
        -O2

@run-pcsx2-no-reset: build-ps2
    echo - Running in PCSX2...
    {{join("pcsx2", "pcsx2-qt")}} \
        -batch \
        -earlyconsolelog \
        -logfile log.txt \
        -elf {{_ps2_elf}}

@run-pcsx2: build-ps2 run-pcsx2-no-reset
    printf '\033\143'