use crate::texture::Atlas;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Data {
    pub atlas: Atlas,
}
