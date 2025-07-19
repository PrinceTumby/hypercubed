set shell := ["bash", "-uc"]
set windows-shell := ["cmd.exe", "/c"]

@_default:
    cd crates/minecraft_client_vk_shaders/builder && cargo build
    cargo build
