set shell := ["bash", "-uc"]
set windows-shell := ["cmd.exe", "/c"]

set ignore-comments

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

@build-vk-shaders:
    echo - Compiling Rust Vulkan shaders...
    cd crates/hypercubed_vk_shaders/builder && cargo build
    echo - Compiling Slang Vulkan shaders...
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/block_face.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o block_face_vertex_bda.spv \
        -entry block_face_vertex_bda
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/block_face.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o block_face_fragment_bda.spv \
        -entry block_face_fragment_bda
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/tinted_block_face.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o tinted_block_face_vertex_bda.spv \
        -entry tinted_block_face_vertex_bda
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/tinted_block_face.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o tinted_block_face_fragment_bda.spv \
        -entry tinted_block_face_fragment_bda
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/custom_block.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o custom_block_vertex_bda.spv \
        -entry custom_block_vertex_bda
    cd crates/hypercubed_vk_shaders && slangc \
        src/chunk/custom_block.slang \
        -profile spirv_1_5 \
        -fvk-use-entrypoint-name \
        -o custom_block_fragment_bda.spv \
        -entry custom_block_fragment_bda
    echo - Done!

# Winit (default)

@run-winit-vulkan *BUILD_FLAGS: build-vk-shaders
    echo - Compiling and running hypercubed...
    cargo run \
        --bin hypercubed \
        --no-default-features \
        --features=platform_winit,graphics_backend_vulkan \
        {{BUILD_FLAGS}}

@build-winit-vulkan *BUILD_FLAGS: build-vk-shaders
    echo - Compiling hypercubed...
    cargo build \
        --bin hypercubed \
        --no-default-features \
        --features=platform_winit,graphics_backend_vulkan \
        {{BUILD_FLAGS}}

# Linux DRM

default_linux_drm_arch := if os() == "linux" { arch() } else { "x86_64" }

@check-linux-drm arch=default_linux_drm_arch:
    cargo check \
        --bin hypercubed \
        --no-default-features \
        --features=platform_linux_drm,graphics_backend_opengl \
        {{ if os() == "linux" { "" } else { "--target" + arch + "-unknown-linux-gnu" } }}

@build-linux-drm arch=default_linux_drm_arch:
    cargo build \
        --bin hypercubed \
        --no-default-features \
        --features=platform_linux_drm,graphics_backend_opengl \
        {{ if os() == "linux" { "" } else { "--target" + arch + "-unknown-linux-gnu" } }}
