cfg_if::cfg_if! {
    if #[cfg(feature = "platform_ps2")] {
        pub mod ps2;
        pub use ps2::exports::*;
    } else if #[cfg(feature = "platform_opengl_mac_tiger")] {
        pub mod opengl_mac_tiger;
        pub use opengl_mac_tiger::exports::*;
    } else if #[cfg(feature = "platform_w2c2_opengl_mac")] {
        pub mod w2c2_opengl_mac;
        pub use w2c2_opengl_mac::exports::*;
    }
}

#[cfg(feature = "use_embedded_cache")]
pub fn get_embedded_cache() -> resources::block::EmbeddedCache {
    // use portable_std::prelude::*;
    // use miniz_oxide::inflate::core::{decompress, DecompressorOxide, inflate_flags};
    // use miniz_oxide::inflate::TINFLStatus;

    // static EMBEDDED_CACHE_COMPRESSED_BYTES: &'static [u8] =
    // include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cache.bincode.zlib"));

    // dbg!();
    // struct CacheDecompressor {
    // decompressor: DecompressorOxide,
    // data_window: Vec<u8>,
    // window_pos: usize,
    // read_pos: usize,
    // }

    // impl CacheDecompressor {
    // pub fn new() -> Self {
    // Self {
    // decompressor: DecompressorOxide::new(),
    // data_window: vec![0; 65536],
    // window_pos: 0,
    // read_pos: 0,
    // }
    // }
    // }

    // impl bincode::de::read::Reader for CacheDecompressor {
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
    // &EMBEDDED_CACHE_COMPRESSED_BYTES[self.read_pos..],
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
    // // dbg!(self.read_pos);
    // // dbg!(self.window_pos);
    // Ok(())
    // }
    // }

    // let embedded_cache = bincode::decode_from_reader(
    // CacheDecompressor::new(),
    // bincode::config::standard(),
    // )
    // .unwrap();

    // FIXME: We need to fix up the above code, this uses too much memory
    static EMBEDDED_CACHE_COMPRESSED_BYTES: &'static [u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cache.bincode.zlib"));

    let bytes =
        miniz_oxide::inflate::decompress_to_vec_zlib(EMBEDDED_CACHE_COMPRESSED_BYTES).unwrap();
    let (embedded_cache, bytes_read) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(bytes_read, bytes.len());

    // static EMBEDDED_CACHE_BYTES: &'static [u8] =
    // include_bytes!("../../temp/embedded_cache.bincode");
    // let (embedded_cache, bytes_read) = bincode::decode_from_slice(
    // EMBEDDED_CACHE_BYTES,
    // bincode::config::standard(),
    // )
    // .unwrap();
    // assert_eq!(bytes_read, EMBEDDED_CACHE_BYTES.len());

    embedded_cache
}

#[cfg(feature = "full_std")]
pub use std::net;

cfg_if::cfg_if! {
    if #[cfg(feature = "mini_std")] {
        pub type StrongRng = rand::rngs::ThreadRng;
    } else {
        pub type StrongRng = rand::rngs::StdRng;
    }
}

pub fn new_strong_rng() -> StrongRng {
    #[cfg(feature = "mini_std")]
    {
        rand::thread_rng()
    }
    #[cfg(not(feature = "mini_std"))]
    {
        // TODO: We really need to make this more secure! Sample some timers or something to
        // mix in some random data.
        let mut per_hasher_seed = 0;
        let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
        per_hasher_seed = stack_ptr;
        todo!()
    }
}
