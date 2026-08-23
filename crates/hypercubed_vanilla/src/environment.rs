use anyhow::Context;
use resources::identifier;
use resources::texture::RawTexture;
use resources::environment::Data as EnvironmentData;

pub fn load_data() -> anyhow::Result<EnvironmentData> {
    let moon_phases_texture =
        RawTexture::load_from_resource(&identifier!("environment/moon_phases"))
            .context("Error while loading moon texture")?;
    let sun_texture = RawTexture::load_from_resource(&identifier!("environment/sun"))
        .context("Error while loading sun texture")?;
    Ok(EnvironmentData {
        moon_phases_texture,
        sun_texture,
    })
}
