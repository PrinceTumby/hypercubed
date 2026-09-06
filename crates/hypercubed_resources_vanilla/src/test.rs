// This file was generated from "oak.bbmodel" using the Hypercubed Blockbench plugin.

#![rustfmt::skip]

use hypercubed_core::types::PercentageF32;
use resources::Identifier;

pub struct UvStorage {
    pub base_north: [u16; 4],
    pub base_east: [u16; 4],
    pub base_south: [u16; 4],
    pub base_west: [u16; 4],
    pub base_up: [u16; 4],
    pub base_down: [u16; 4],
    pub back_north: [u16; 4],
    pub back_east: [u16; 4],
    pub back_south: [u16; 4],
    pub back_west: [u16; 4],
    pub back_up: [u16; 4],
    pub back_down: [u16; 4],
    pub left_north: [u16; 4],
    pub left_east: [u16; 4],
    pub left_south: [u16; 4],
    pub left_west: [u16; 4],
    pub left_up: [u16; 4],
    pub left_down: [u16; 4],
    pub right_north: [u16; 4],
    pub right_east: [u16; 4],
    pub right_south: [u16; 4],
    pub right_west: [u16; 4],
    pub right_up: [u16; 4],
    pub right_down: [u16; 4],
    pub front_north: [u16; 4],
    pub front_east: [u16; 4],
    pub front_south: [u16; 4],
    pub front_west: [u16; 4],
    pub front_up: [u16; 4],
    pub front_down: [u16; 4],
}

impl UvStorage {
    pub fn load_from(atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        Ok(Self {
            base_north: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/north")).unwrap())
                .context("Error while loading texture part \"base/north\"")?
                .basic_or_first_frame_uvs(),
            base_east: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/east")).unwrap())
                .context("Error while loading texture part \"base/east\"")?
                .basic_or_first_frame_uvs(),
            base_south: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/south")).unwrap())
                .context("Error while loading texture part \"base/south\"")?
                .basic_or_first_frame_uvs(),
            base_west: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/west")).unwrap())
                .context("Error while loading texture part \"base/west\"")?
                .basic_or_first_frame_uvs(),
            base_up: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/up")).unwrap())
                .context("Error while loading texture part \"base/up\"")?
                .basic_or_first_frame_uvs(),
            base_down: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/down")).unwrap())
                .context("Error while loading texture part \"base/down\"")?
                .basic_or_first_frame_uvs(),
            back_north: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/north")).unwrap())
                .context("Error while loading texture part \"back/north\"")?
                .basic_or_first_frame_uvs(),
            back_east: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/east")).unwrap())
                .context("Error while loading texture part \"back/east\"")?
                .basic_or_first_frame_uvs(),
            back_south: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/south")).unwrap())
                .context("Error while loading texture part \"back/south\"")?
                .basic_or_first_frame_uvs(),
            back_west: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/west")).unwrap())
                .context("Error while loading texture part \"back/west\"")?
                .basic_or_first_frame_uvs(),
            back_up: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/up")).unwrap())
                .context("Error while loading texture part \"back/up\"")?
                .basic_or_first_frame_uvs(),
            back_down: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/down")).unwrap())
                .context("Error while loading texture part \"back/down\"")?
                .basic_or_first_frame_uvs(),
            left_north: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/north")).unwrap())
                .context("Error while loading texture part \"left/north\"")?
                .basic_or_first_frame_uvs(),
            left_east: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/east")).unwrap())
                .context("Error while loading texture part \"left/east\"")?
                .basic_or_first_frame_uvs(),
            left_south: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/south")).unwrap())
                .context("Error while loading texture part \"left/south\"")?
                .basic_or_first_frame_uvs(),
            left_west: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/west")).unwrap())
                .context("Error while loading texture part \"left/west\"")?
                .basic_or_first_frame_uvs(),
            left_up: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/up")).unwrap())
                .context("Error while loading texture part \"left/up\"")?
                .basic_or_first_frame_uvs(),
            left_down: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/down")).unwrap())
                .context("Error while loading texture part \"left/down\"")?
                .basic_or_first_frame_uvs(),
            right_north: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/north")).unwrap())
                .context("Error while loading texture part \"right/north\"")?
                .basic_or_first_frame_uvs(),
            right_east: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/east")).unwrap())
                .context("Error while loading texture part \"right/east\"")?
                .basic_or_first_frame_uvs(),
            right_south: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/south")).unwrap())
                .context("Error while loading texture part \"right/south\"")?
                .basic_or_first_frame_uvs(),
            right_west: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/west")).unwrap())
                .context("Error while loading texture part \"right/west\"")?
                .basic_or_first_frame_uvs(),
            right_up: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/up")).unwrap())
                .context("Error while loading texture part \"right/up\"")?
                .basic_or_first_frame_uvs(),
            right_down: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/down")).unwrap())
                .context("Error while loading texture part \"right/down\"")?
                .basic_or_first_frame_uvs(),
            front_north: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/north")).unwrap())
                .context("Error while loading texture part \"front/north\"")?
                .basic_or_first_frame_uvs(),
            front_east: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/east")).unwrap())
                .context("Error while loading texture part \"front/east\"")?
                .basic_or_first_frame_uvs(),
            front_south: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/south")).unwrap())
                .context("Error while loading texture part \"front/south\"")?
                .basic_or_first_frame_uvs(),
            front_west: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/west")).unwrap())
                .context("Error while loading texture part \"front/west\"")?
                .basic_or_first_frame_uvs(),
            front_up: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/up")).unwrap())
                .context("Error while loading texture part \"front/up\"")?
                .basic_or_first_frame_uvs(),
            front_down: atlas
                .get_texture(&Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/down")).unwrap())
                .context("Error while loading texture part \"front/down\"")?
                .basic_or_first_frame_uvs(),
        })
    }
}

pub fn load_textures(atlas_builder: &mut resources::texture::AtlasBuilder) -> anyhow::Result<()> {
    atlas_builder.load_texture_parts(
        &Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak")).unwrap(),
        [
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/north")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/east")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/south")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/west")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.4609375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/up")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.484375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/base/down")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/north")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/east")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/south")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.171875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.3125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/west")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.171875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/up")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/back/down")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/north")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/east")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/south")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/west")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.46875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/up")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/left/down")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/north")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/east")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.46875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/south")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/west")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/up")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/right/down")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/north")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.28125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/east")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/south")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/west")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/up")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                ],
            ),
            (
                Identifier::parse(&format!("hypercubed_vanilla:entity/boat/oak/front/down")).unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.28125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                ],
            ),
        ],
    );
}