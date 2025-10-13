use spirv_builder::{Capability, MetadataPrintout, SpirvBuilder};

fn main() {
    println!("cargo::rerun-if-changed=../src");
    let compiled = match SpirvBuilder::new("crate_redirect", "spirv-unknown-vulkan1.2")
        .capability(Capability::Int8)
        .capability(Capability::Int16)
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
}
