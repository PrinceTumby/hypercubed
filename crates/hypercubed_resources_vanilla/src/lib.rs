pub mod blocks;
pub mod environment;
pub mod entity;

use anyhow::Context;

use resources::GameResourceData;

/// Loads the vanilla game data from either a vanilla client `minecraft.jar` file, or from an
/// extracted `assets`.
/// Can be used either to load the game data at run time, or from a build script to then embed the
/// resource data at compile time.
pub fn load_data() -> anyhow::Result<GameResourceData> {
    let block_data = blocks::load_data().context("Error while loading block resource data")?;
    let environment_data =
        environment::load_data().context("Error while loading environment resource data")?;
    let entity_data = entity::load_data().context("Error while loading entity resource data")?;
    Ok(GameResourceData {
        block_data,
        environment_data,
        entity_data,
    })
}
