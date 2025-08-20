cfg_if::cfg_if! {
    if #[cfg(feature = "platform_ps2")] {
        pub mod ps2;
        pub use ps2::exports::*;
    }
}

#[cfg(feature = "use_embedded_cache")]
pub fn get_embedded_cache() -> resources::block::EmbeddedCache {
    static EMBEDDED_CACHE_COMPRESSED_BYTES: &'static [u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cache.bincode.zlib"));
    // TODO: Write an adaptor that can stream bytes from this to bincode, so we don't have to
    //       allocate a Vec.
    let embedded_cache_bytes = miniz_oxide::inflate::decompress_to_vec_zlib(
        EMBEDDED_CACHE_COMPRESSED_BYTES
    )
    .unwrap();
    let (embedded_cache, num_bytes_read) = bincode::decode_from_slice(
        &embedded_cache_bytes,
        bincode::config::standard(),
    )
    .unwrap();
    debug_assert!(num_bytes_read == embedded_cache_bytes.len());
    embedded_cache
}

#[cfg(feature = "std")]
pub use std::net;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub type StrongRng = rand::rngs::ThreadRng;
    } else {
        pub type StrongRng = rand::rngs::StdRng;
    }
}

pub fn new_strong_rng() -> StrongRng {
    #[cfg(feature = "std")]
    {
        rand::thread_rng()
    }
    #[cfg(not(feature = "std"))]
    {
        // TODO: We really need to make this more secure! Sample some timers or something to
        // mix in some random data.
        let mut per_hasher_seed = 0;
        let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
        per_hasher_seed = stack_ptr;
        todo!()
    }
}
