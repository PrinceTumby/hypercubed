#![allow(unexpected_cfgs)]
#![allow(clippy::too_many_arguments)]

pub mod chunk;
pub mod debug;
pub mod egui;
pub mod sky;

/// Chunk descriptor set binding indices.
/// Must be manually kept in sync with shader function definitions.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommonDescriptorSetIdxs {
    RenderInfo = 0,
    Lightmap = 1,
    BlockItemAtlasCombinedImageSampler = 2,
    CustomBlockFaces = 3,
    SunCombinedImageSampler = 4,
    MoonPhasesCombinedImageSampler = 5,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(target_arch = "spirv"), derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct RawRenderInfo {
    pub view_matrix: [[f32; 4]; 4],
    pub sky_matrix: [[f32; 4]; 4],
    /// `[1.0 / screen.width, 1.0 / screen.height]`
    pub recip_screen_size: [f32; 2],
    /// `[1.0 / atlas.width, 1.0 / atlas.height]`
    pub recip_block_item_atlas_size: [f32; 2],
    pub face_matrices: [[[f32; 4]; 3]; 6],
    /// `time_of_day.rem_euclid(192_000.0)`
    pub time_of_day: f32,
    pub star_brightness: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderInfo {
    pub view_matrix: spirv_std::glam::Mat4,
    pub sky_matrix: spirv_std::glam::Mat4,
    pub recip_screen_size: spirv_std::glam::Vec2,
    pub recip_block_item_atlas_size: spirv_std::glam::Vec2,
    pub face_matrices: [spirv_std::glam::Mat3A; 6],
    pub time_of_day: f32,
    pub star_brightness: f32,
}

pub type RawAtlasImage = spirv_std::image::Image!(
    2D,
    format = rgba8,
    // type=f32,
    depth = false,
    sampled = false,
);

pub type LightmapImage = spirv_std::image::Image!(
    buffer,
    type = f32,
    depth = false,
    sampled = true,
);

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "spirv"))] {
        #[macro_export]
        #[allow(unused)]
        macro_rules! cpu_dbg {
            ($($exprs:expr),* $(,)?) => {
                dbg!($($exprs),*)
            };
        }
    } else {
        #[macro_export]
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

#[cfg(not(target_arch = "spirv"))]
pub fn cpu_dummy_ray_query() -> spirv_std::ray_tracing::RayQuery {
    unsafe { core::mem::transmute(0u32) }
}

#[macro_export]
macro_rules! make_ray_query_or_cpu_dummy {
    (let mut $name:ident) => {
        ::cfg_if::cfg_if! {
            if #[cfg(target_arch = "spirv")] {
                ::spirv_std::ray_query!(let mut $name);
            } else {
                let mut $name = &mut $crate::cpu_dummy_ray_query();
            }
        }
    };
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
            module!("chunk", "block_face", "vertex");
            module!("chunk", "block_face", "fragment");
            module!("chunk", "tinted_block_face", "vertex");
            module!("chunk", "tinted_block_face", "fragment");
            module!("chunk", "custom_block", "vertex");
            module!("chunk", "custom_block", "fragment");
            module!("sky", "sun", "vertex");
            module!("sky", "sun", "fragment");
            module!("sky", "moon", "vertex");
            module!("sky", "moon", "fragment");
            module!("sky", "star", "vertex");
            module!("sky", "star", "fragment");
            module!("egui", "vertex");
            module!("egui", "fragment");
            module!("debug", "point", "vertex");
            module!("debug", "point", "fragment");
            module!("debug", "line", "vertex");
            module!("debug", "line", "fragment");
            module!("debug", "triangle", "vertex");
            module!("debug", "triangle", "fragment");
            map.shrink_to_fit();
            map
        };
    }

    pub fn get_entry_point(
        device: &Arc<VulkanDevice>,
        entry_point: &'static str,
    ) -> VulkanEntryPoint {
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
