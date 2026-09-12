use anyhow::Context;
use hypercubed_entity_models as models;
use resources::entity::Data as EntityData;

pub fn load_data() -> anyhow::Result<EntityData> {
    let mut atlas_builder = resources::texture::AtlasBuilder::new(
        [128; 2],
        resources::texture::AtlasAllocatorOptions {
            small_size_threshold: 16,
            large_size_threshold: 16,
            ..Default::default()
        },
    );
    load_model_textures(&mut atlas_builder).context("Error while loading entity model textures")?;
    let atlas = atlas_builder.finish();
    log::debug!(
        "Entity atlas dimensions - {}x{}px",
        atlas.width,
        atlas.height
    );
    Ok(EntityData { atlas })
}

fn load_model_textures(atlas_builder: &mut resources::texture::AtlasBuilder) -> anyhow::Result<()> {
    models::oak_boat::load_textures(atlas_builder)
        .context("Error while loading oak boat textures")?;
    models::pig::load_textures(atlas_builder)
        .context("Error while loading pig textures")?;
    Ok(())
}
