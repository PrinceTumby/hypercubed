use crate::texture::RawTexture;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub moon_phases_texture: RawTexture,
    pub sun_texture: RawTexture,
}
