fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Generate embedded cache.
    if std::env::var("CARGO_FEATURE_USE_EMBEDDED_CACHE").is_ok() {
        let resource_data = if std::env::var("CARGO_FEATURE_EMBED_VANILLA_CACHE").is_ok() {
            resources::block::load_vanilla_resource_data().unwrap()
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
