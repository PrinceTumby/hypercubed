#![allow(unexpected_cfgs)]

pub mod chunk_rc;
pub mod debug;
pub mod egui;

pub type RawAtlasImage = spirv_std::image::Image!(
    2D,
    format = rgba8,
    // type=f32,
    depth = false,
    sampled = false,
);

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "spirv"))] {
        #[allow(unused)]
        macro_rules! cpu_dbg {
            ($($exprs:expr),* $(,)?) => {
                dbg!($($exprs),*)
            };
        }
    } else {
        #[allow(unused)]
        macro_rules! cpu_dbg {
            ($expr:expr $(,)?) => {
                $expr
            };
            ($($exprs:expr),* $(,)?) => {
                ($($exprs,)*)
            };
        }
    }
}

#[cfg(feature = "vulkano")]
pub mod shader_modules {
    use ahash::AHashMap;
    use anyhow::Context;
    use lazy_static::lazy_static;
    use std::sync::{Arc, Mutex};
    use vulkan_prelude::*;

    // We have to do a little hacking here to get the embedded SPIR-V module to be u32 aligned, for the
    // bytemuck slice cast.

    struct SpvAlignedBytes<Bytes: ?Sized> {
        _align: [u32; 0],
        bytes: Bytes,
    }

    lazy_static! {
        static ref RAW_MODULES: AHashMap<&'static str, &'static SpvAlignedBytes<[u8]>> = {
            let mut map = AHashMap::new();
            macro_rules! generate_entry_point_name {
                ($x:literal) => {
                    $x
                };
                ($x:literal, $($xs:literal),+) => {
                    concat!($x, "::", generate_entry_point_name!($($xs),+))
                };
            }
            macro_rules! generate_file_name {
                ($x:literal) => {
                    $x
                };
                ($x:literal, $($xs:literal),+) => {
                    concat!($x, "-", generate_file_name!($($xs),+))
                };
            }
            macro_rules! module {
                ($($name_segments:literal),+) => {{
                    static MODULE_BYTES: &'static SpvAlignedBytes<[u8]> = &SpvAlignedBytes {
                        _align: [],
                        bytes: *include_bytes!(concat!(
                            "../shader-",
                            generate_file_name!($($name_segments),+),
                            ".spv",
                        )),
                    };
                    map.insert(
                        concat!(
                            "shader::",
                            generate_entry_point_name!($($name_segments),+),
                        ),
                        MODULE_BYTES,
                    );
                }};
            }
            module!("chunk_rc", "block_face", "vertex");
            module!("chunk_rc", "block_face", "fragment");
            module!("chunk_rc", "tinted_block_face", "vertex");
            module!("chunk_rc", "tinted_block_face", "fragment");
            module!("chunk_rc", "custom_block", "vertex");
            module!("chunk_rc", "custom_block", "fragment");
            module!("egui", "vertex");
            module!("egui", "fragment");
            module!("chunk_rc", "rc_compute", "raytrace_debug");
            module!("chunk_rc", "rc_compute", "single_pass_update");
            module!("chunk_rc", "rc_compute", "update_all_cascades");
            module!("chunk_rc", "rc_compute", "update_cascade_0");
            module!("chunk_rc", "rc_compute", "update_cascade_1");
            module!("debug", "point", "vertex");
            module!("debug", "point", "fragment");
            module!("debug", "line", "vertex");
            module!("debug", "line", "fragment");
            module!("debug", "triangle", "vertex");
            module!("debug", "triangle", "fragment");
            // module!("render_raytraced");
            map.shrink_to_fit();
            map
        };
    }

    pub fn get_entry_point(device: &Arc<VulkanDevice>, entry_point: &'static str) -> VulkanEntryPoint {
        lazy_static! {
            static ref LOADED_MODULES: Mutex<AHashMap<&'static str, Arc<VulkanShaderModule>>> =
                Mutex::new(AHashMap::new());
        }
        let vk_entry_point = LOADED_MODULES
            .lock()
            .unwrap()
            .entry(entry_point)
            .or_insert_with(|| unsafe {
                let raw_module = RAW_MODULES
                    .get(entry_point)
                    .with_context(|| format!("Failed to find shader entry point \"{entry_point}\""))
                    .unwrap();
                VulkanShaderModule::new(
                    device,
                    &VulkanShaderModuleCreateInfo {
                        code: bytemuck::cast_slice::<u8, u32>(&raw_module.bytes),
                        _ne: vulkano_non_exhaustive(),
                    },
                )
                .expect("Failed to load SPIR-V shader module")
            })
            .entry_point(entry_point);
        match vk_entry_point {
            Some(vk_entry_point) => vk_entry_point,
            None => panic!("Shader entry point {entry_point} wasn't found"),
        }
    }

    pub fn shader_stage_from_entry_point<'a>(
        temp_entry_point_store: &'a mut Option<VulkanEntryPoint>,
        device: &Arc<VulkanDevice>,
        entry_point: &'static str,
    ) -> VulkanPipelineShaderStageCreateInfo<'a> {
        *temp_entry_point_store = Some(get_entry_point(device, entry_point));
        VulkanPipelineShaderStageCreateInfo {
            flags: VulkanPipelineShaderStageCreateFlags::default(),
            entry_point: temp_entry_point_store.as_ref().unwrap(),
            required_subgroup_size: None,
            // HACK: See prelude.
            _ne: vulkano_non_exhaustive(),
        }
    }
}