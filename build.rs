fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Generate embedded cache.
    if std::env::var("CARGO_FEATURE_USE_EMBEDDED_CACHE").is_ok() {
        let resource_data = if std::env::var("CARGO_FEATURE_EMBED_VANILLA_CACHE").is_ok() {
            resources::GameResourceData::load_vanilla_data()
                .expect("Error while building vanilla embedded resource data cache")
        } else {
            panic!(concat!(
                "Feature `use_embedded_cache` requires another feature to be enabled to ",
                "specify which resource data cache to generate and embed. ",
                "See feature `embed_vanilla_cache`.",
            ));
        };
        let out_dir = std::env::var("OUT_DIR").unwrap();
        zlib_postcard_encode_to_file(
            resource_data,
            format!("{out_dir}/embedded_cache.postcard.zlib"),
        );
    }
    // Compile WESL shaders.
    #[cfg(feature = "graphics_backend_wgpu")]
    {
        let wesl_builder = wesl::Wesl::new("src/graphics/backend_wgpu/shaders");
        wesl_builder.build_artifact(&"package::block_face".parse().unwrap(), "block_face");
        wesl_builder.build_artifact(
            &"package::tinted_block_face".parse().unwrap(),
            "tinted_block_face",
        );
        wesl_builder.build_artifact(&"package::custom_block".parse().unwrap(), "custom_block");
        wesl_builder.build_artifact(&"package::sun".parse().unwrap(), "sun");
        wesl_builder.build_artifact(&"package::moon".parse().unwrap(), "moon");
        wesl_builder.build_artifact(&"package::star".parse().unwrap(), "star");
        wesl_builder.build_artifact(&"package::egui".parse().unwrap(), "egui");
        wesl_builder.build_artifact(&"package::debug_point".parse().unwrap(), "debug_point");
        wesl_builder.build_artifact(&"package::debug_line".parse().unwrap(), "debug_line");
        wesl_builder.build_artifact(
            &"package::debug_triangle".parse().unwrap(),
            "debug_triangle",
        );
    }
}

fn zlib_postcard_encode_to_file<P: AsRef<std::path::Path>, E: serde::Serialize>(value: E, path: P) {
    let bytes = postcard::to_stdvec(&value).unwrap();
    verify_base_bytes(&bytes);
    let compressed_bytes = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
    verify_compressed_bytes(&compressed_bytes);
    std::fs::write(path, compressed_bytes).unwrap();
}

fn verify_base_bytes(bytes: &[u8]) {
    let _resource_data: resources::block::ResourceData = postcard::from_bytes(bytes).unwrap();
}

fn verify_compressed_bytes(compressed_bytes: &[u8]) {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(compressed_bytes).unwrap();
    let _resource_data: resources::block::ResourceData = postcard::from_bytes(&bytes).unwrap();
}
