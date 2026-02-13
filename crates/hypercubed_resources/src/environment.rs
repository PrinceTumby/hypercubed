use crate::identifier;
use crate::texture::RawTexture;
use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ResourceData {
    pub moon_phases_texture: RawTexture,
    pub sun_texture: RawTexture,
}

#[cfg(feature = "std")]
impl ResourceData {
    pub fn load_vanilla_data() -> anyhow::Result<Self> {
        let moon_phases_texture =
            RawTexture::load_from_resource(&identifier!("environment/moon_phases"))
                .context("Error while loading moon texture")?;
        let sun_texture = RawTexture::load_from_resource(&identifier!("environment/sun"))
            .context("Error while loading sun texture")?;
        Ok(Self {
            moon_phases_texture,
            sun_texture,
        })
    }
}
