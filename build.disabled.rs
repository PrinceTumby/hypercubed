fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=src/client/graphics/backend_vulkan/shaders/src");
    if std::env::var("CARGO_FEATURE_GRAPHICS_BACKEND_VULKAN").is_ok() {
		// FIXME: Cargo seems to completely ignore the version specifier when it's given in a
		// 		  build script? Why?
        let status = std::process::Command::new("cargo")
            .args(["+nightly-2024-04-24", "build", "--release"])
            .current_dir(std::fs::canonicalize(
                "src/client/graphics/backend_vulkan/shaders/dummy_builder",
            )?)
            .status()?;
        assert!(status.success(), "Shader compilation failed");
    }
    Ok(())
}
