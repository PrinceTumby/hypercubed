cfg_select! {
    feature = "platform_winit" => {
        pub mod winit;
        pub use winit::exports::*;
    }
    feature = "platform_linux_drm" => {
        pub mod linux_drm;
        pub use linux_drm::exports::*;
    }
    _ => {
        compile_error!("A platform feature must be enabled.");
    }
}

pub fn load_resource_data() -> anyhow::Result<resources::GameResourceData> {
    cfg_select! {
        feature = "use_embedded_cache" => {
            Ok(extract_embedded_resource_data_cache())
        }
        feature = "full_std" => {
            use anyhow::{Context, anyhow};
            static COMPRESSED_POSTCARD: &[u8] = include_bytes!(concat!(
                env!("OUT_DIR"),
                "/vanilla_blocks_registration_list.postcard.zlib",
            ));
            let uncompressed_postcard = miniz_oxide::inflate::decompress_to_vec_zlib(COMPRESSED_POSTCARD)
                .map_err(|err| anyhow!("Failed to decompress vanilla block postcard data - {err:#}"))?;
            let registrations: Vec<resources::block::Registration> = {
                let start_time = std::time::Instant::now();
                let registrations: Vec<resources::block::Registration> = postcard::from_bytes(&uncompressed_postcard)
                    .context("Error while deserialising registration list postcard data")?;
                log::debug!(
                    "Block registration list load time: {:?}",
                    std::time::Instant::now() - start_time
                );
                registrations
            };
            resources::GameResourceData::load_vanilla_data(registrations)
        }
        _ => {
            compile_error!(concat!(
                "Either a full standard library must be available, or an embedded cache must be ",
                "generated at compile time. ",
                "See the `full_std` and `embeded_vanilla_cache` features."
            ));
        }
    }
}

#[cfg(feature = "use_embedded_cache")]
fn extract_embedded_resource_data_cache() -> resources::GameResourceData {
    // FIXME: Fix up old implementation of this, current one uses too much memory.
    static EMBEDDED_CACHE_COMPRESSED_BYTES: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_cache.postcard.zlib"));
    let bytes =
        miniz_oxide::inflate::decompress_to_vec_zlib(EMBEDDED_CACHE_COMPRESSED_BYTES).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

cfg_select! {
    feature = "full_std" => {
        #[allow(unused)]
        pub(crate) use std::{dbg, eprintln, print, println};
        pub use std::net;
    }
}

cfg_select! {
    feature = "mini_std" => {
        pub type StrongRng = rand::rngs::ThreadRng;
    }
    _ => {
        pub type StrongRng = rand::rngs::StdRng;
    }
}

pub fn new_strong_rng() -> StrongRng {
    cfg_select! {
        feature = "full_std" => {
            rand::rng()
        }
        _ => {{
            // TODO: We really need to make this more secure! Sample some timers or something to
            // mix in some random data.
            let mut per_hasher_seed = 0;
            let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
            per_hasher_seed = stack_ptr;
            todo!()
        }}
    }
}
