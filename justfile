set shell := ["bash", "-uc"]
set windows-shell := ["cmd.exe", "/c"]

@build-with-vk-shaders *BUILD_FLAGS:
    echo - Compiling Vulkan shaders...
    cd crates/minecraft_client_vk_shaders/builder && cargo build
    echo - Compiling client...
    cargo build {{BUILD_FLAGS}}
