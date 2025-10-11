fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Generate embedded cache
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
    // Generate Mac W2C2 OpenGL FFI bindings
    if std::env::var("CARGO_FEATURE_PLATFORM_W2C2_OPENGL_MAC").is_ok() {
        // Create C bindings directory
        let mut c_out_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        c_out_dir.push("target");
        c_out_dir.push(std::env::var("TARGET").unwrap());
        c_out_dir.push(std::env::var("PROFILE").unwrap());
        c_out_dir.push("w2c2_generated_bindings");
        std::fs::create_dir_all(&c_out_dir).unwrap();
        // Create Rust bindings directory
        let mut rust_out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        rust_out_dir.push("w2c2_generated_bindings");
        std::fs::create_dir_all(&rust_out_dir).unwrap();
        // Compile OpenGL bindings
        println!("cargo::rerun-if-changed=src/client/graphics/backend_w2c2_opengl_mac/gl.kdl");
        w2c2_binding_generator::compile_document(
            "src/client/graphics/backend_w2c2_opengl_mac/gl.kdl",
            c_out_dir,
            rust_out_dir,
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

// fn verify_compressed_bytes(compressed_bytes: &[u8]) {
// use miniz_oxide::inflate::core::{decompress, DecompressorOxide, inflate_flags};
// use miniz_oxide::inflate::TINFLStatus;

// struct CacheDecompressor<'a> {
// decompressor: DecompressorOxide,
// data_window: Vec<u8>,
// window_pos: usize,
// read_data: &'a [u8],
// read_pos: usize,
// }

// impl<'a> CacheDecompressor<'a> {
// pub fn new(read_data: &'a [u8]) -> Self {
// Self {
// decompressor: DecompressorOxide::new(),
// data_window: vec![0; 65536],
// window_pos: 0,
// read_data,
// read_pos: 0,
// }
// }
// }

// impl bincode::de::read::Reader for CacheDecompressor<'_> {
// fn read(&mut self, bytes: &mut [u8]) -> Result<(), bincode::error::DecodeError> {
// // println!("Read!");
// let mut out_bytes_pos = 0;
// while out_bytes_pos < bytes.len() {
// let num_bytes_to_write = usize::min(
// bytes.len() - out_bytes_pos,
// self.data_window.len() - self.window_pos,
// );
// // println!("Reading {num_bytes_to_write} bytes...");
// let (status, bytes_read, bytes_written) = decompress(
// &mut self.decompressor,
// &self.read_data[self.read_pos..],
// &mut self.data_window[..self.window_pos + num_bytes_to_write],
// self.window_pos,
// inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
// | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
// );
// self.read_pos += bytes_read;
// match status {
// TINFLStatus::Done => assert!(out_bytes_pos + bytes_written == bytes.len()),
// TINFLStatus::HasMoreOutput => {}
// other => panic!("Cache decompress error: {other:?}"),
// }
// bytes[out_bytes_pos..out_bytes_pos + bytes_written]
// .copy_from_slice(&self.data_window[self.window_pos..self.window_pos + bytes_written]);
// out_bytes_pos += bytes_written;
// if self.window_pos + bytes_written > 32768 {
// let overflow_bytes = self.window_pos + bytes_written - 32768;
// self.data_window.copy_within(
// overflow_bytes..self.window_pos + overflow_bytes,
// 0,
// );
// self.window_pos = 32768;
// } else {
// self.window_pos += bytes_written;
// }
// }
// // println!("cargo::warning=read_pos: {}", self.read_pos);
// // dbg!(self.window_pos);
// Ok(())
// }
// }

// let _embedded_cache: resources::block::EmbeddedCache = bincode::decode_from_reader(
// CacheDecompressor::new(compressed_bytes),
// bincode::config::standard(),
// )
// .unwrap();
// dbg!(_embedded_cache.block_registry);
// }
