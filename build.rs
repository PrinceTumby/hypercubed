fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Generate embedded cache.
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

fn zlib_bincode_encode_to_file<P: AsRef<std::path::Path>, E: bincode::Encode>(value: E, path: P) {
    let bytes = bincode::encode_to_vec(value, bincode::config::standard()).unwrap();
    verify_base_bytes(&bytes);
    let compressed_bytes = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
    verify_compressed_bytes(&compressed_bytes);
    std::fs::write(path, compressed_bytes).unwrap();
}

fn verify_base_bytes(bytes: &[u8]) {
    let (_embedded_cache, bytes_read): (resources::block::EmbeddedCache, usize) =
        bincode::decode_from_slice(bytes, bincode::config::standard()).unwrap();
    assert_eq!(bytes_read, bytes.len());
}

fn verify_compressed_bytes(compressed_bytes: &[u8]) {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(compressed_bytes).unwrap();
    let (_embedded_cache, bytes_read): (resources::block::EmbeddedCache, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(bytes_read, bytes.len());
}
