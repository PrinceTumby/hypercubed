set shell := ["bash", "-uc"]
set windows-shell := ["cmd.exe", "/c"]

set ignore-comments
set dotenv-load

os := os()
os_family := os_family()

# Command replacements
wsl := if os_family == "windows" { "wsl -d Ubuntu" } else { "wsl.exe -d Ubuntu" }
wsl24 := if os_family == "windows" { "wsl -d Ubuntu-24.04" } else { "" }
rm := if os_family == "windows" { "del" } else { "rm" }
rmdir := if os_family == "windows" { "rmdir /s /q" } else { "rm -r" }
copy := if os_family == "windows" { "1>NUL copy /b /y" } else { "cp" }
copydir := if os_family == "windows" { "1>NUL xcopy /c /q /e /i" } else { "cp -r" }
mkdir_create_parents := if os_family == "windows" { "mkdir" } else { "mkdir -p" }
silence_stderr := if os_family == "windows" { "2>NUL" } else { "2>/dev/null" }
ignore_error := if os_family == "windows" { "|| rem" } else { "|| true" }

@_default:
    just --list

@build-with-vk-shaders *BUILD_FLAGS:
    echo - Compiling Vulkan shaders...
    cd crates/hypercubed_vk_shaders/builder && cargo build
    echo - Compiling client...
    cargo build {{BUILD_FLAGS}}


# Playstation 2

_ps2_out_dir := join("target", "mipsel-sony-ps2", "dev-ps2")
_ps2_unstripped_bin := join(_ps2_out_dir, "hypercubed_unstripped")
_ps2_elf := join(_ps2_out_dir, "hypercubed.elf")

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
    cargo +nightly-custom build \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache \
        --profile=dev-ps2 \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem
    {{wsl}} \
        /usr/local/ps2dev/ee/bin/mips64r5900el-ps2-elf-gcc \
        src/platform/ps2/entry.s \
        src/platform/ps2/syscalls.s \
        src/platform/ps2/asm_helpers.s \
        -Wl,--gc-sections \
        target/mipsel-sony-ps2/dev-ps2/libhypercubed.a \
        -mabi=64 \
        -Wl,--no-warn-mismatch \
        -T targets/ps2.ld \
        -o target/mipsel-sony-ps2/dev-ps2/hypercubed_unstripped.elf \
        -ffreestanding \
        -nostdlib \
        -lgcc \
        -O2
    {{wsl}} \
        /usr/local/ps2dev/ee/bin/mips64r5900el-ps2-elf-strip \
        --strip-debug \
        target/mipsel-sony-ps2/dev-ps2/hypercubed_unstripped.elf \
        -o target/mipsel-sony-ps2/dev-ps2/hypercubed.elf

@build-release-ps2:
    echo - Compiling...
    cargo +nightly-custom build \
        --lib \
        --target targets/mipsel-sony-ps2.json \
        --no-default-features \
        --features=platform_ps2,embed_vanilla_cache \
        --profile=release-ps2 \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem
    {{wsl}} \
        /usr/local/ps2dev/ee/bin/mips64r5900el-ps2-elf-gcc \
        src/platform/ps2/entry.s \
        src/platform/ps2/syscalls.s \
        src/platform/ps2/asm_helpers.s \
        -Wl,--gc-sections \
        target/mipsel-sony-ps2/release-ps2/libhypercubed.a \
        -mabi=64 \
        -Wl,--no-warn-mismatch \
        -T targets/ps2.ld \
        -o target/mipsel-sony-ps2/release-ps2/hypercubed.elf \
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

# iMac Core Duo

@check-imac-tiger:
    echo - Compiling...
    cargo +nightly-custom check \
        --lib \
        --target targets/i686-apple-darwin8.json \
        --no-default-features \
        --features=platform_opengl_mac_tiger,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem

@build-imac-tiger:
    echo - Compiling...
    cargo +nightly-custom build \
        --lib \
        --target targets/i686-apple-darwin8.json \
        --no-default-features \
        --features=platform_opengl_mac_tiger,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc \
        -Zbuild-std-features=compiler-builtins-mem

@build-imac-tiger-transfer-remote: build-imac-tiger
    echo - Copying to remote...
    {{join("scripts", "imac_run_psftp_link_only.bat")}} {{env("IMAC_PASSWORD")}}
    {{join("scripts", "imac_run_plink_link_only.bat")}} {{env("IMAC_PASSWORD")}}

@check-imac-w2c2:
    cargo +nightly-2025-08-25 check \
        --target targets/ppc_mac/wasm32-wasip1.json \
        --no-default-features \
        --features=platform_w2c2_opengl_mac,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc,std,panic_abort \
        -Zbuild-std-features=compiler-builtins-mem

@build-imac-w2c2-wasm:
    echo - Compiling...
    cargo +nightly-2025-08-25 rustc \
        --bin hypercubed \
        --target targets/ppc_mac/wasm32-wasip1.json \
        --no-default-features \
        --features=platform_w2c2_opengl_mac,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc,std,panic_abort \
        -Zbuild-std-features=compiler-builtins-mem \
        -- \
        -Zwasi-exec-model=reactor \

@build-imac-w2c2-remote: build-imac-w2c2-wasm
    echo - Compiling WASM to C...
    cd {{join("target", "wasm32-wasip1", "debug")}} && \
        {{rm}} *.c *.h \
        {{silence_stderr}} {{ignore_error}}
    cd {{join("target", "wasm32-wasip1", "debug")}} && w2c2 \
        -d sectcreate2 \
        -f 500 \
        hypercubed.wasm \
        hypercubed.c
    echo - Copying files to remote...
    {{join("scripts", "imac_run_psftp.bat")}} {{env("IMAC_PASSWORD")}}
    echo - Compiling C on remote...
    {{join("scripts", "imac_run_plink.bat")}} {{env("IMAC_PASSWORD")}}

# iBook G3

@check-ibook:
    cargo +nightly-2025-08-25 check \
        --target targets/ppc_mac/wasm32-wasip1.json \
        --no-default-features \
        --features=platform_w2c2_opengl_mac,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc,std,panic_abort \
        -Zbuild-std-features=compiler-builtins-mem

@build-ibook-wasm:
    echo - Compiling...
    cargo +nightly-2025-08-25 rustc \
        --bin hypercubed \
        --target targets/ppc_mac/wasm32-wasip1.json \
        --no-default-features \
        --features=platform_w2c2_opengl_mac,embed_vanilla_cache \
        -Zbuild-std=core,compiler_builtins,alloc,std,panic_abort \
        -Zbuild-std-features=compiler-builtins-mem \
        -- \
        -Zwasi-exec-model=reactor \

@build-ibook-w2c2-remote: build-ibook-wasm
    echo - Compiling WASM to C...
    cd {{join("target", "wasm32-wasip1", "debug")}} && \
        {{rm}} *.c *.h \
        {{silence_stderr}} {{ignore_error}}
    w2c2 \
        -d sectcreate2 \
        -f 500 \
        target/wasm32-wasip1/debug/hypercubed.wasm \
        target/wasm32-wasip1/debug/hypercubed.c
    echo - Copying files to remote...
    ibook_run_psftp.bat {{env("IBOOK_PASSWORD")}}
    echo - Compiling C on remote...
    ibook_run_plink.bat {{env("IBOOK_PASSWORD")}}
