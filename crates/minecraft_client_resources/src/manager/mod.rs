mod internal_overlay;

use super::Identifier;
use ahash::{AHashMap, AHashSet};
use anyhow::{anyhow, bail};
use lazy_static::lazy_static;
use portable_std::{Arc, Cow};
use std::io::{Cursor, Read};
use std::path::PathBuf;
#[cfg(feature = "std")]
use std::sync::RwLock;
use zip::ZipArchive;

pub fn get_resource_file(
    resource_type: &ResourceType,
    identifier: &Identifier,
) -> anyhow::Result<Cow<'static, [u8]>> {
    let overlay_set = GLOBAL_OVERLAY_SET.read().unwrap();
    let set = match resource_type {
        &ResourceType::Blockstate => &overlay_set.blockstates,
        &ResourceType::Model => &overlay_set.models,
        &ResourceType::Texture | &ResourceType::TextureMeta => &overlay_set.textures,
        &ResourceType::TextureLuma => &overlay_set.luma_textures,
    };
    if let Some(&filesystem_index) = set.get(identifier) {
        let filesystem = &overlay_set.filesystems[filesystem_index];
        filesystem.get(resource_type, identifier)
    } else {
        let mut path = match &*MAIN_FILESYSTEM {
            MainFilesystem::JarFile(_zip_archive) => PathBuf::from("assets"),
            MainFilesystem::AssetsFolder(path_prefix) => path_prefix.clone(),
        };
        path.push(identifier.namespace.as_ref());
        path.push(match resource_type {
            &ResourceType::Blockstate => "blockstates",
            &ResourceType::Model => "models",
            &ResourceType::Texture | &ResourceType::TextureMeta => "textures",
            &ResourceType::TextureLuma => bail!("Luma textures not supported by vanilla assets"),
        });
        for segment in &identifier.path_prefix_segments {
            path.push(segment.as_ref());
        }
        path.push(identifier.path_name.as_ref());
        path.set_extension(match resource_type {
            &ResourceType::Blockstate | &ResourceType::Model => "json",
            &ResourceType::Texture => "png",
            &ResourceType::TextureMeta => "png.mcmeta",
            &ResourceType::TextureLuma => unimplemented!(),
        });
        match &*MAIN_FILESYSTEM {
            MainFilesystem::JarFile(zip_archive) => {
                // `ZipArchive` is currently cheap to clone.
                let mut zip_archive = zip_archive.clone();
                let index = zip_archive
                    .index_for_path(&path)
                    .ok_or_else(|| anyhow!("Unknown path {path:?}"))?;
                let mut file_in_zip = zip_archive.by_index(index)?;
                let mut buffer = Vec::new();
                file_in_zip.read_to_end(&mut buffer)?;
                Ok(buffer.into())
            }
            MainFilesystem::AssetsFolder(_path_prefix) => Ok(Cow::Owned(std::fs::read(&path)?)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceType {
    Blockstate,
    Model,
    Texture,
    TextureMeta,
    // NOTE: RADIANCE CASCADES
    TextureLuma,
}

lazy_static! {
    static ref MAIN_FILESYSTEM: MainFilesystem = {
        // TODO: Make this configurable.
        if let Ok(jar_bytes) = std::fs::read("minecraft.jar")
            && let jar_cursor = Cursor::new(jar_bytes.into())
            && let Ok(zip_archive) = ZipArchive::new(jar_cursor)
        {
            MainFilesystem::JarFile(zip_archive)
        } else if std::fs::read_dir("assets").is_ok() {
            MainFilesystem::AssetsFolder(PathBuf::from("assets"))
        } else {
            panic!(concat!(
                "Cannot find assets source! ",
                "Expected to find `minecraft.jar` or an `assets` directory",
            ));
        }
    };

    static ref GLOBAL_OVERLAY_SET: RwLock<GlobalOverlays> = {
        let internal_blockstates: AHashSet<_> =
            internal_overlay::BLOCKSTATES.keys().cloned().collect();
        let internal_models: AHashSet<_> = internal_overlay::MODELS.keys().cloned().collect();
        let internal_textures: AHashSet<_> = internal_overlay::TEXTURES.keys().cloned().collect();
        let internal_luma_textures: AHashSet<_> =
            internal_overlay::TEXTURE_LUMAS.keys().cloned().collect();
        let internal_filesystem_overlay = FilesystemOverlay {
            filesystem: Box::new(internal_overlay::InternalOverlayFilesystem),
            blockstates: internal_blockstates,
            models: internal_models,
            textures: internal_textures,
            luma_textures: internal_luma_textures,
        };
        RwLock::new(GlobalOverlays::new(
            [internal_filesystem_overlay].into_iter(),
        ))
    };
}

trait Filesystem: Send + Sync {
    fn get(
        &self,
        resource_type: &ResourceType,
        identifier: &Identifier,
    ) -> anyhow::Result<Cow<'static, [u8]>>;
}

enum MainFilesystem {
    JarFile(ZipArchive<Cursor<Arc<[u8]>>>),
    AssetsFolder(PathBuf),
}

struct GlobalOverlays {
    pub filesystems: Vec<Box<dyn Filesystem>>,
    pub blockstates: AHashMap<Identifier, usize>,
    pub models: AHashMap<Identifier, usize>,
    pub textures: AHashMap<Identifier, usize>,
    pub luma_textures: AHashMap<Identifier, usize>,
}

impl GlobalOverlays {
    pub fn new(filesystem_overlays: impl Iterator<Item = FilesystemOverlay>) -> Self {
        let mut filesystems = Vec::new();
        let mut blockstates = AHashMap::new();
        let mut models = AHashMap::new();
        let mut textures = AHashMap::new();
        let mut luma_textures = AHashMap::new();
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
            let luma_texture_entries = filesystem_overlay
                .luma_textures
                .into_iter()
                .map(|identifier| (identifier, filesystem_index));
            blockstates.extend(blockstate_entries);
            models.extend(model_entries);
            textures.extend(texture_entries);
            luma_textures.extend(luma_texture_entries);
        }
        Self {
            filesystems,
            blockstates,
            models,
            textures,
            luma_textures,
        }
    }
}

struct FilesystemOverlay {
    pub filesystem: Box<dyn Filesystem>,
    pub blockstates: AHashSet<Identifier>,
    pub models: AHashSet<Identifier>,
    pub textures: AHashSet<Identifier>,
    pub luma_textures: AHashSet<Identifier>,
}
