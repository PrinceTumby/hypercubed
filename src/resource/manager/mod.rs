mod internal_overlay;

use super::Identifier;
use ahash::{AHashMap, AHashSet};
use lazy_static::lazy_static;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::RwLock;

pub fn get_resource_file(
    resource_type: &ResourceType,
    identifier: &Identifier,
) -> anyhow::Result<Cow<'static, [u8]>> {
    let overlay_set = GLOBAL_OVERLAY_SET.read().unwrap();
    let set = match resource_type {
        &ResourceType::Blockstate => &overlay_set.blockstates,
        &ResourceType::Model => &overlay_set.models,
        &ResourceType::Texture => &overlay_set.textures,
    };
    if let Some(&filesystem_index) = set.get(identifier) {
        let filesystem = &overlay_set.filesystems[filesystem_index];
        filesystem.get(resource_type, identifier)
    } else {
        let mut path = PathBuf::new();
        path.push("assets");
        path.push(identifier.namespace.as_ref());
        path.push(match resource_type {
            &ResourceType::Blockstate => "blockstates",
            &ResourceType::Model => "models",
            &ResourceType::Texture => "textures",
        });
        for segment in &identifier.path_prefix_segments {
            path.push(segment.as_ref());
        }
        path.push(identifier.path_name.as_ref());
        path.set_extension(match resource_type {
            &ResourceType::Blockstate | &ResourceType::Model => "json",
            &ResourceType::Texture => "png",
        });
        Ok(Cow::Owned(std::fs::read(&path)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    Blockstate,
    Model,
    Texture,
}

lazy_static! {
    static ref GLOBAL_OVERLAY_SET: RwLock<GlobalOverlays> = {
        let internal_blockstates: AHashSet<_> =
            internal_overlay::BLOCKSTATES.keys().cloned().collect();
        let internal_models: AHashSet<_> = internal_overlay::MODELS.keys().cloned().collect();
        let internal_textures: AHashSet<_> = internal_overlay::TEXTURES.keys().cloned().collect();
        let internal_filesystem_overlay = FilesystemOverlay {
            filesystem: Box::new(internal_overlay::InternalOverlayFilesystem),
            blockstates: internal_blockstates,
            models: internal_models,
            textures: internal_textures,
        };
        RwLock::new(GlobalOverlays::new(
            [internal_filesystem_overlay].into_iter(),
        ))
    };
}

struct GlobalOverlays {
    pub filesystems: Vec<Box<dyn Filesystem>>,
    pub blockstates: AHashMap<Identifier, usize>,
    pub models: AHashMap<Identifier, usize>,
    pub textures: AHashMap<Identifier, usize>,
}

impl GlobalOverlays {
    pub fn new(filesystem_overlays: impl Iterator<Item = FilesystemOverlay>) -> Self {
        let mut filesystems = Vec::new();
        let mut blockstates = AHashMap::new();
        let mut models = AHashMap::new();
        let mut textures = AHashMap::new();
        for filesystem_overlay in filesystem_overlays {
            let filesystem_index = filesystems.len();
            filesystems.push(filesystem_overlay.filesystem);
            let blockstate_entries = filesystem_overlay
                .blockstates
                .into_iter()
                .map(|identifier| (identifier, filesystem_index));
            let model_entries = filesystem_overlay
                .models
                .into_iter()
                .map(|identifier| (identifier, filesystem_index));
            let texture_entries = filesystem_overlay
                .textures
                .into_iter()
                .map(|identifier| (identifier, filesystem_index));
            blockstates.extend(blockstate_entries);
            models.extend(model_entries);
            textures.extend(texture_entries);
        }
        Self {
            filesystems,
            blockstates,
            models,
            textures,
        }
    }
}

struct FilesystemOverlay {
    pub filesystem: Box<dyn Filesystem>,
    pub blockstates: AHashSet<Identifier>,
    pub models: AHashSet<Identifier>,
    pub textures: AHashSet<Identifier>,
}

trait Filesystem: Send + Sync {
    fn get(
        &self,
        resource_type: &ResourceType,
        identifier: &Identifier,
    ) -> anyhow::Result<Cow<'static, [u8]>>;
}
