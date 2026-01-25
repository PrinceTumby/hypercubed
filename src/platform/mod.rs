cfg_if::cfg_if! {
    if #[cfg(feature = "platform_winit")] {
        pub mod winit;
        pub use winit::exports::*;
    } else if #[cfg(feature = "platform_linux_drm")] {
        pub mod linux_drm;
        pub use linux_drm::exports::*;
    } else {
        compile_error!("A platform feature must be enabled.");
    }
}

pub fn load_resource_data() -> anyhow::Result<resources::block::ResourceData> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "use_embedded_cache")] {
            Ok(extract_embedded_resource_data_cache())
        } else {
            resources::block::load_vanilla_resource_data()
        }
    }
}

#[cfg(feature = "use_embedded_cache")]
fn extract_embedded_resource_data_cache() -> resources::block::ResourceData {
    // FIXME: Fix up old implementation of this, current one uses too much memory.
    static EMBEDDED_CACHE_COMPRESSED_BYTES: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cache.postcard.zlib"));
    let bytes =
        miniz_oxide::inflate::decompress_to_vec_zlib(EMBEDDED_CACHE_COMPRESSED_BYTES).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

cfg_if::cfg_if! {
    if #[cfg(feature = "full_std")] {
        #[allow(unused)]
        pub(crate) use std::{dbg, eprintln, print, println};
        pub use std::net;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "mini_std")] {
        pub type StrongRng = rand::rngs::ThreadRng;
    } else {
        pub type StrongRng = rand::rngs::StdRng;
    }
}

pub fn new_strong_rng() -> StrongRng {
    cfg_if::cfg_if! {
        if #[cfg(feature = "full_std")] {
            rand::rng()
        } else {{
            // TODO: We really need to make this more secure! Sample some timers or something to
            // mix in some random data.
            let mut per_hasher_seed = 0;
            let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
            per_hasher_seed = stack_ptr;
            todo!()
        }}
    }
}
