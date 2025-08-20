fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_FEATURE_USE_EMBEDDED_CACHE").is_ok() {
        let embedded_cache = if std::env::var("CARGO_FEATURE_EMBED_VANILLA_CACHE").is_ok() {
            resources::block::generate_vanilla_embedded_cache().unwrap()
        } else {
            panic!(concat!(
                "Feature `use_embedded_cache` requires another feature to be enabled to ",
                "specify which cache to generate and embed. ",
                "See feature `embed_vanilla_cache`.",
            ));
        };
        let out_dir = std::env::var("OUT_DIR").unwrap();
        zlib_bincode_encode_to_file(
            embedded_cache,
            format!("{out_dir}/embedded_cache.bincode.zlib"),
        );
    }
}

fn zlib_bincode_encode_to_file<P: AsRef<std::path::Path>, E: bincode::Encode>(
    value: E,
    path: P,
) {
    let bytes = bincode::encode_to_vec(value, bincode::config::standard()).unwrap();
    let compressed_bytes = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
    std::fs::write(path, compressed_bytes).unwrap();
}
