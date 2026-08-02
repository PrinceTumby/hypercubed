pub mod chunk;
pub mod debug;

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
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
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

pub mod shader_modules {
    use ahash::AHashMap;
    use anyhow::Context;
    use lazy_static::lazy_static;
    use std::sync::{Arc, Mutex};
    use vulkan_prelude::*;

    // We have to do a little hacking here to get the embedded SPIR-V module to be u32 aligned, for
    // the bytemuck slice cast.

    struct SpvAlignedBytes<Bytes: ?Sized> {
        _align: [u32; 0],
        bytes: Bytes,
    }

    lazy_static! {
        static ref RAW_MODULES: AHashMap<&'static str, &'static SpvAlignedBytes<[u8]>> = {
            let mut map = AHashMap::new();
            macro_rules! generate_module_name {
                ($x:literal) => {
                    $x
                };
                ($x:literal, $($xs:literal),+) => {
                    concat!($x, "::", generate_module_name!($($xs),+))
                };
            }
            macro_rules! generate_file_stem {
                ($x:literal) => {
                    $x
                };
                ($x:literal, $($xs:literal),+) => {
                    concat!($x, "-", generate_file_stem!($($xs),+))
                };
            }
            macro_rules! module {
                ($($name_segments:literal),+) => {{
                    static MODULE_BYTES: &'static SpvAlignedBytes<[u8]> = &SpvAlignedBytes {
                        _align: [],
                        bytes: *include_bytes!(concat!(
                            env!("OUT_DIR"),
                            "/",
                            generate_file_stem!($($name_segments),+),
                            ".spv",
                        )),
                    };
                    map.insert(
                        generate_module_name!($($name_segments),+),
                        MODULE_BYTES,
                    );
                }};
            }
            // Chunk.
            module!("chunk", "block_face");
            module!("chunk", "tinted_block_face");
            module!("chunk", "custom_block");
            // Sky.
            module!("sky", "sun");
            module!("sky", "moon");
            module!("sky", "star");
            // `egui`.
            module!("egui");
            // Debug graphics.
            module!("debug", "point");
            module!("debug", "line");
            module!("debug", "triangle");
            map.shrink_to_fit();
            map
        };
    }

    pub fn get_entry_point(
        device: &Arc<VulkanDevice>,
        module: &'static str,
        entry_point: &'static str,
    ) -> VulkanEntryPoint {
        lazy_static! {
            static ref LOADED_MODULES: Mutex<AHashMap<&'static str, Arc<VulkanShaderModule>>> =
                Mutex::new(AHashMap::new());
        }
        LOADED_MODULES
            .lock()
            .unwrap()
            .entry(module)
            .or_insert_with(|| unsafe {
                let raw_module = RAW_MODULES
                    .get(module)
                    .with_context(|| {
                        let available_module_names: Vec<&&str> = RAW_MODULES.keys().collect();
                        format!(
                            "Failed to find SPIR-V module \"{}\", available modules - {:?}",
                            module,
                            available_module_names,
                        )
                    })
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
            .entry_point(entry_point)
            .with_context(|| {
                format!(
                    "Failed to find entry point \"{}\" in SPIR-V module \"{}\"",
                    entry_point, module,
                )
            })
            .unwrap()
    }

    pub fn shader_stage_from_entry_point<'a>(
        temp_entry_point_store: &'a mut Option<VulkanEntryPoint>,
        device: &Arc<VulkanDevice>,
        module: &'static str,
        entry_point: &'static str,
    ) -> VulkanPipelineShaderStageCreateInfo<'a> {
        *temp_entry_point_store = Some(get_entry_point(device, module, entry_point));
        VulkanPipelineShaderStageCreateInfo {
            flags: VulkanPipelineShaderStageCreateFlags::default(),
            entry_point: temp_entry_point_store.as_ref().unwrap(),
            required_subgroup_size: None,
            // HACK: See prelude.
            _ne: vulkano_non_exhaustive(),
        }
    }
}
