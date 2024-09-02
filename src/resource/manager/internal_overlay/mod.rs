use super::{Filesystem, ResourceType};
use crate::identifier;
use crate::resource::Identifier;
use ahash::AHashMap;
use anyhow::anyhow;
use lazy_static::lazy_static;
use std::borrow::Cow;

macro_rules! make_entry_macro {
    ($map:ident, $subpath:expr, $extension:expr) => {
        macro_rules! entry {
            ($name:expr) => {
                $map.insert(
                    identifier!($name),
                    include_bytes!(concat!("minecraft/", $subpath, "/", $name, $extension))
                        .as_ref(),
                )
            };
        }
    };
}

lazy_static! {
    pub static ref BLOCKSTATES: AHashMap<Identifier, &'static [u8]> = {
        let mut map = AHashMap::new();
        make_entry_macro!(map, "blockstates", ".json");
        entry!("white_bed");
        entry!("orange_bed");
        entry!("magenta_bed");
        entry!("light_blue_bed");
        entry!("yellow_bed");
        entry!("lime_bed");
        entry!("pink_bed");
        entry!("gray_bed");
        entry!("light_gray_bed");
        entry!("cyan_bed");
        entry!("purple_bed");
        entry!("blue_bed");
        entry!("brown_bed");
        entry!("green_bed");
        entry!("red_bed");
        entry!("black_bed");
        map.shrink_to_fit();
        map
    };
    pub static ref MODELS: AHashMap<Identifier, &'static [u8]> = {
        let mut map = AHashMap::new();
        make_entry_macro!(map, "models", ".json");
        entry!("block/template_bed_head");
        entry!("block/template_bed_foot");
        entry!("block/white_bed_head");
        entry!("block/white_bed_foot");
        entry!("block/orange_bed_head");
        entry!("block/orange_bed_foot");
        entry!("block/magenta_bed_head");
        entry!("block/magenta_bed_foot");
        entry!("block/light_blue_bed_head");
        entry!("block/light_blue_bed_foot");
        entry!("block/yellow_bed_head");
        entry!("block/yellow_bed_foot");
        entry!("block/lime_bed_head");
        entry!("block/lime_bed_foot");
        entry!("block/pink_bed_head");
        entry!("block/pink_bed_foot");
        entry!("block/gray_bed_head");
        entry!("block/gray_bed_foot");
        entry!("block/light_gray_bed_head");
        entry!("block/light_gray_bed_foot");
        entry!("block/cyan_bed_head");
        entry!("block/cyan_bed_foot");
        entry!("block/purple_bed_head");
        entry!("block/purple_bed_foot");
        entry!("block/blue_bed_head");
        entry!("block/blue_bed_foot");
        entry!("block/brown_bed_head");
        entry!("block/brown_bed_foot");
        entry!("block/green_bed_head");
        entry!("block/green_bed_foot");
        entry!("block/red_bed_head");
        entry!("block/red_bed_foot");
        entry!("block/black_bed_head");
        entry!("block/black_bed_foot");
        entry!("block/heavy_core");
        entry!("block/moving_piston");
        map.shrink_to_fit();
        map
    };
    pub static ref TEXTURES: AHashMap<Identifier, &'static [u8]> = AHashMap::new();
    pub static ref TEXTURE_METAS: AHashMap<Identifier, &'static [u8]> = AHashMap::new();
}

pub struct InternalOverlayFilesystem;

impl Filesystem for InternalOverlayFilesystem {
    fn get(
        &self,
        resource_type: &ResourceType,
        identifier: &Identifier,
    ) -> anyhow::Result<Cow<'static, [u8]>> {
        match resource_type {
            ResourceType::Blockstate => &*BLOCKSTATES,
            ResourceType::Model => &*MODELS,
            ResourceType::Texture => &*TEXTURES,
            ResourceType::TextureMeta => &*TEXTURE_METAS,
        }
        .get(identifier)
        .map(|&file| Cow::Borrowed(file))
        .ok_or_else(|| anyhow!("file not found in internal overlay filesystem"))
    }
}
