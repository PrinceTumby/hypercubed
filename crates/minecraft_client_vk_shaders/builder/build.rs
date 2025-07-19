use spirv_builder::{MetadataPrintout, SpirvBuilder, Capability};

fn main() {
    println!("cargo::rerun-if-changed=../src");
    let compiled = match SpirvBuilder::new("crate_redirect", "spirv-unknown-vulkan1.2")
        .capability(Capability::Int8)
        .capability(Capability::Int16)
        .capability(Capability::RayQueryKHR)
        .extension("SPV_KHR_ray_query")
        .multimodule(true)
        .print_metadata(MetadataPrintout::None)
        // .shader_panic_strategy(spirv_builder::ShaderPanicStrategy::DebugPrintfThenExit {
        //     print_inputs: false,
        //     print_backtrace: false,
        // })
        .build()
    {
        Ok(result) => result,
        Err(err) => panic!("{err}"),
    };
    let modules = compiled.module.unwrap_multi();
    for (_entry_point, path) in modules.iter() {
        let mut dest_path = std::path::PathBuf::new();
        dest_path.push("..");
        dest_path.push(path.file_name().unwrap());
        std::fs::copy(path, &dest_path).unwrap();
    }
    // let status = std::process::Command::new("spirv-link")
    //     .args(modules
    //         .iter()
    //         // .filter(|(entry_point, _)| *entry_point != "chunk_rc::rc_compute::raytrace_debug")
    //         .map(|(_, path)| path.file_name().unwrap()))
    //     .args(["-o", "generated.spv"])
    //     .current_dir(std::fs::canonicalize("..").unwrap())
    //     .status()
    //     .unwrap();
    // assert!(status.success(), "Shader linking failed");
}
