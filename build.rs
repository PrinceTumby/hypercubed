fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // Generate embedded cache.
    if std::env::var("CARGO_FEATURE_USE_EMBEDDED_CACHE").is_ok() {
        let resource_data = if std::env::var("CARGO_FEATURE_EMBED_VANILLA_CACHE").is_ok() {
            hypercubed_vanilla::load_data()
                .expect("Error while building vanilla embedded resource data cache")
        } else {
            panic!(concat!(
                "Feature `use_embedded_cache` requires another feature to be enabled to ",
                "specify which resource data cache to generate and embed. ",
                "See feature `embed_vanilla_cache`.",
            ));
        };
        zlib_postcard_encode_to_file(
            format!("{out_dir}/embedded_cache.postcard.zlib"),
            resource_data,
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

fn zlib_postcard_encode_to_file<'de, P, T>(path: P, value: T)
where
    P: AsRef<std::path::Path>,
    T: serde::Serialize + serde::Deserialize<'de>,
{
    let bytes = postcard::to_stdvec(&value).unwrap();
    let compressed_bytes = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
    std::fs::write(path, compressed_bytes).unwrap();
}
