// This file was generated from "oak.bbmodel" using the Hypercubed Blockbench plugin.

#![allow(unused)]

use anyhow::Context as _;
use hypercubed_core::types::PercentageF32;
use nalgebra::{Matrix4, Point3, Vector3};
use resources::Identifier;

use super::{EntityRenderQuad as Quad, EntityRenderVertex as Vertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/north").unwrap())
                .context("Error while loading texture part \"base/north\"")?
                .basic_or_first_frame_uvs(),
            base_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/east").unwrap())
                .context("Error while loading texture part \"base/east\"")?
                .basic_or_first_frame_uvs(),
            base_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/south").unwrap())
                .context("Error while loading texture part \"base/south\"")?
                .basic_or_first_frame_uvs(),
            base_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/west").unwrap())
                .context("Error while loading texture part \"base/west\"")?
                .basic_or_first_frame_uvs(),
            base_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/up").unwrap())
                .context("Error while loading texture part \"base/up\"")?
                .basic_or_first_frame_uvs(),
            base_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/down").unwrap())
                .context("Error while loading texture part \"base/down\"")?
                .basic_or_first_frame_uvs(),
            back_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/north").unwrap())
                .context("Error while loading texture part \"back/north\"")?
                .basic_or_first_frame_uvs(),
            back_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/east").unwrap())
                .context("Error while loading texture part \"back/east\"")?
                .basic_or_first_frame_uvs(),
            back_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/south").unwrap())
                .context("Error while loading texture part \"back/south\"")?
                .basic_or_first_frame_uvs(),
            back_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/west").unwrap())
                .context("Error while loading texture part \"back/west\"")?
                .basic_or_first_frame_uvs(),
            back_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/up").unwrap())
                .context("Error while loading texture part \"back/up\"")?
                .basic_or_first_frame_uvs(),
            back_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/down").unwrap())
                .context("Error while loading texture part \"back/down\"")?
                .basic_or_first_frame_uvs(),
            left_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/north").unwrap())
                .context("Error while loading texture part \"left/north\"")?
                .basic_or_first_frame_uvs(),
            left_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/east").unwrap())
                .context("Error while loading texture part \"left/east\"")?
                .basic_or_first_frame_uvs(),
            left_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/south").unwrap())
                .context("Error while loading texture part \"left/south\"")?
                .basic_or_first_frame_uvs(),
            left_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/west").unwrap())
                .context("Error while loading texture part \"left/west\"")?
                .basic_or_first_frame_uvs(),
            left_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/up").unwrap())
                .context("Error while loading texture part \"left/up\"")?
                .basic_or_first_frame_uvs(),
            left_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/down").unwrap())
                .context("Error while loading texture part \"left/down\"")?
                .basic_or_first_frame_uvs(),
            right_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/north").unwrap())
                .context("Error while loading texture part \"right/north\"")?
                .basic_or_first_frame_uvs(),
            right_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/east").unwrap())
                .context("Error while loading texture part \"right/east\"")?
                .basic_or_first_frame_uvs(),
            right_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/south").unwrap())
                .context("Error while loading texture part \"right/south\"")?
                .basic_or_first_frame_uvs(),
            right_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/west").unwrap())
                .context("Error while loading texture part \"right/west\"")?
                .basic_or_first_frame_uvs(),
            right_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/up").unwrap())
                .context("Error while loading texture part \"right/up\"")?
                .basic_or_first_frame_uvs(),
            right_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/down").unwrap())
                .context("Error while loading texture part \"right/down\"")?
                .basic_or_first_frame_uvs(),
            front_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/north").unwrap())
                .context("Error while loading texture part \"front/north\"")?
                .basic_or_first_frame_uvs(),
            front_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/east").unwrap())
                .context("Error while loading texture part \"front/east\"")?
                .basic_or_first_frame_uvs(),
            front_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/south").unwrap())
                .context("Error while loading texture part \"front/south\"")?
                .basic_or_first_frame_uvs(),
            front_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/west").unwrap())
                .context("Error while loading texture part \"front/west\"")?
                .basic_or_first_frame_uvs(),
            front_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/up").unwrap())
                .context("Error while loading texture part \"front/up\"")?
                .basic_or_first_frame_uvs(),
            front_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/down").unwrap())
                .context("Error while loading texture part \"front/down\"")?
                .basic_or_first_frame_uvs(),
        })
    }
}

#[cfg(feature = "std")]
pub fn load_textures(atlas_builder: &mut resources::texture::AtlasBuilder) -> anyhow::Result<()> {
    atlas_builder.load_texture_parts(
        &Identifier::parse("minecraft:entity/boat/oak").unwrap(),
        [
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.4609375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.484375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/base/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.046875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.2421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.171875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.3125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.171875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/back/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.296875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.46875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.796875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/left/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.703125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.46875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.671875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/right/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.234375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.578125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.28125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.546875_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.015625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.140625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/boat/oak/front/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.15625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.421875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.28125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.453125_f32),
                ],
            ),
        ],
    )?;
    Ok(())
}

pub fn render(
    out_quads: &mut Vec<Quad>,
    uv_storage: &UvStorage,
    pos: Point3<f32>,
    yaw_degrees: f32,
    head_yaw_degrees: f32,
    pitch_degrees: f32,
) {
    let matrix: Matrix4<f32> = Matrix4::from_scaled_axis(-Vector3::y() * yaw_degrees.to_radians())
        .append_translation(&pos.coords);
    const BASE_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.5_f32, 0.1875_f32, 0.875_f32),
        Point3::new(0.5_f32, 0.1875_f32, -0.875_f32),
        Point3::new(0.5_f32, 0_f32, 0.875_f32),
        Point3::new(0.5_f32, 0_f32, -0.875_f32),
        Point3::new(-0.5_f32, 0.1875_f32, -0.875_f32),
        Point3::new(-0.5_f32, 0.1875_f32, 0.875_f32),
        Point3::new(-0.5_f32, 0_f32, -0.875_f32),
        Point3::new(-0.5_f32, 0_f32, 0.875_f32),
    ];
    let base_positions: [[f32; 3]; 8] =
        BASE_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const BACK_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.5625_f32, 0.5625_f32, 1_f32),
        Point3::new(0.5625_f32, 0.5625_f32, 0.875_f32),
        Point3::new(0.5625_f32, 0.1875_f32, 1_f32),
        Point3::new(0.5625_f32, 0.1875_f32, 0.875_f32),
        Point3::new(-0.5625_f32, 0.5625_f32, 0.875_f32),
        Point3::new(-0.5625_f32, 0.5625_f32, 1_f32),
        Point3::new(-0.5625_f32, 0.1875_f32, 0.875_f32),
        Point3::new(-0.5625_f32, 0.1875_f32, 1_f32),
    ];
    let back_positions: [[f32; 3]; 8] =
        BACK_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const LEFT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(-0.5_f32, 0.5625_f32, 0.875_f32),
        Point3::new(-0.5_f32, 0.5625_f32, -0.875_f32),
        Point3::new(-0.5_f32, 0.1875_f32, 0.875_f32),
        Point3::new(-0.5_f32, 0.1875_f32, -0.875_f32),
        Point3::new(-0.625_f32, 0.5625_f32, -0.875_f32),
        Point3::new(-0.625_f32, 0.5625_f32, 0.875_f32),
        Point3::new(-0.625_f32, 0.1875_f32, -0.875_f32),
        Point3::new(-0.625_f32, 0.1875_f32, 0.875_f32),
    ];
    let left_positions: [[f32; 3]; 8] =
        LEFT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const RIGHT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.625_f32, 0.5625_f32, 0.875_f32),
        Point3::new(0.625_f32, 0.5625_f32, -0.875_f32),
        Point3::new(0.625_f32, 0.1875_f32, 0.875_f32),
        Point3::new(0.625_f32, 0.1875_f32, -0.875_f32),
        Point3::new(0.5_f32, 0.5625_f32, -0.875_f32),
        Point3::new(0.5_f32, 0.5625_f32, 0.875_f32),
        Point3::new(0.5_f32, 0.1875_f32, -0.875_f32),
        Point3::new(0.5_f32, 0.1875_f32, 0.875_f32),
    ];
    let right_positions: [[f32; 3]; 8] =
        RIGHT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const FRONT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.5_f32, 0.5625_f32, -0.875_f32),
        Point3::new(0.5_f32, 0.5625_f32, -1_f32),
        Point3::new(0.5_f32, 0.1875_f32, -0.875_f32),
        Point3::new(0.5_f32, 0.1875_f32, -1_f32),
        Point3::new(-0.5_f32, 0.5625_f32, -1_f32),
        Point3::new(-0.5_f32, 0.5625_f32, -0.875_f32),
        Point3::new(-0.5_f32, 0.1875_f32, -1_f32),
        Point3::new(-0.5_f32, 0.1875_f32, -0.875_f32),
    ];
    let front_positions: [[f32; 3]; 8] =
        FRONT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    out_quads.extend([
        Quad([Vertex { pos: base_positions[1], uv: [uv_storage.base_north[2], uv_storage.base_north[1]] }, Vertex { pos: base_positions[4], uv: [uv_storage.base_north[2], uv_storage.base_north[3]] }, Vertex { pos: base_positions[6], uv: [uv_storage.base_north[0], uv_storage.base_north[3]] }, Vertex { pos: base_positions[3], uv: [uv_storage.base_north[0], uv_storage.base_north[1]] }]),
        Quad([Vertex { pos: base_positions[0], uv: [uv_storage.base_east[0], uv_storage.base_east[1]] }, Vertex { pos: base_positions[1], uv: [uv_storage.base_east[2], uv_storage.base_east[1]] }, Vertex { pos: base_positions[3], uv: [uv_storage.base_east[2], uv_storage.base_east[3]] }, Vertex { pos: base_positions[2], uv: [uv_storage.base_east[0], uv_storage.base_east[3]] }]),
        Quad([Vertex { pos: base_positions[5], uv: [uv_storage.base_south[0], uv_storage.base_south[3]] }, Vertex { pos: base_positions[0], uv: [uv_storage.base_south[0], uv_storage.base_south[1]] }, Vertex { pos: base_positions[2], uv: [uv_storage.base_south[2], uv_storage.base_south[1]] }, Vertex { pos: base_positions[7], uv: [uv_storage.base_south[2], uv_storage.base_south[3]] }]),
        Quad([Vertex { pos: base_positions[4], uv: [uv_storage.base_west[2], uv_storage.base_west[1]] }, Vertex { pos: base_positions[5], uv: [uv_storage.base_west[0], uv_storage.base_west[1]] }, Vertex { pos: base_positions[7], uv: [uv_storage.base_west[0], uv_storage.base_west[3]] }, Vertex { pos: base_positions[6], uv: [uv_storage.base_west[2], uv_storage.base_west[3]] }]),
        Quad([Vertex { pos: base_positions[4], uv: [uv_storage.base_up[0], uv_storage.base_up[3]] }, Vertex { pos: base_positions[1], uv: [uv_storage.base_up[0], uv_storage.base_up[1]] }, Vertex { pos: base_positions[0], uv: [uv_storage.base_up[2], uv_storage.base_up[1]] }, Vertex { pos: base_positions[5], uv: [uv_storage.base_up[2], uv_storage.base_up[3]] }]),
        Quad([Vertex { pos: base_positions[7], uv: [uv_storage.base_down[0], uv_storage.base_down[3]] }, Vertex { pos: base_positions[2], uv: [uv_storage.base_down[0], uv_storage.base_down[1]] }, Vertex { pos: base_positions[3], uv: [uv_storage.base_down[2], uv_storage.base_down[1]] }, Vertex { pos: base_positions[6], uv: [uv_storage.base_down[2], uv_storage.base_down[3]] }]),
        Quad([Vertex { pos: back_positions[1], uv: [uv_storage.back_north[0], uv_storage.back_north[1]] }, Vertex { pos: back_positions[4], uv: [uv_storage.back_north[2], uv_storage.back_north[1]] }, Vertex { pos: back_positions[6], uv: [uv_storage.back_north[2], uv_storage.back_north[3]] }, Vertex { pos: back_positions[3], uv: [uv_storage.back_north[0], uv_storage.back_north[3]] }]),
        Quad([Vertex { pos: back_positions[0], uv: [uv_storage.back_east[0], uv_storage.back_east[1]] }, Vertex { pos: back_positions[1], uv: [uv_storage.back_east[2], uv_storage.back_east[1]] }, Vertex { pos: back_positions[3], uv: [uv_storage.back_east[2], uv_storage.back_east[3]] }, Vertex { pos: back_positions[2], uv: [uv_storage.back_east[0], uv_storage.back_east[3]] }]),
        Quad([Vertex { pos: back_positions[5], uv: [uv_storage.back_south[0], uv_storage.back_south[1]] }, Vertex { pos: back_positions[0], uv: [uv_storage.back_south[2], uv_storage.back_south[1]] }, Vertex { pos: back_positions[2], uv: [uv_storage.back_south[2], uv_storage.back_south[3]] }, Vertex { pos: back_positions[7], uv: [uv_storage.back_south[0], uv_storage.back_south[3]] }]),
        Quad([Vertex { pos: back_positions[4], uv: [uv_storage.back_west[0], uv_storage.back_west[1]] }, Vertex { pos: back_positions[5], uv: [uv_storage.back_west[2], uv_storage.back_west[1]] }, Vertex { pos: back_positions[7], uv: [uv_storage.back_west[2], uv_storage.back_west[3]] }, Vertex { pos: back_positions[6], uv: [uv_storage.back_west[0], uv_storage.back_west[3]] }]),
        Quad([Vertex { pos: back_positions[4], uv: [uv_storage.back_up[2], uv_storage.back_up[3]] }, Vertex { pos: back_positions[1], uv: [uv_storage.back_up[0], uv_storage.back_up[3]] }, Vertex { pos: back_positions[0], uv: [uv_storage.back_up[0], uv_storage.back_up[1]] }, Vertex { pos: back_positions[5], uv: [uv_storage.back_up[2], uv_storage.back_up[1]] }]),
        Quad([Vertex { pos: back_positions[7], uv: [uv_storage.back_down[0], uv_storage.back_down[1]] }, Vertex { pos: back_positions[2], uv: [uv_storage.back_down[2], uv_storage.back_down[1]] }, Vertex { pos: back_positions[3], uv: [uv_storage.back_down[2], uv_storage.back_down[3]] }, Vertex { pos: back_positions[6], uv: [uv_storage.back_down[0], uv_storage.back_down[3]] }]),
        Quad([Vertex { pos: left_positions[1], uv: [uv_storage.left_north[0], uv_storage.left_north[1]] }, Vertex { pos: left_positions[4], uv: [uv_storage.left_north[2], uv_storage.left_north[1]] }, Vertex { pos: left_positions[6], uv: [uv_storage.left_north[2], uv_storage.left_north[3]] }, Vertex { pos: left_positions[3], uv: [uv_storage.left_north[0], uv_storage.left_north[3]] }]),
        Quad([Vertex { pos: left_positions[0], uv: [uv_storage.left_east[0], uv_storage.left_east[1]] }, Vertex { pos: left_positions[1], uv: [uv_storage.left_east[2], uv_storage.left_east[1]] }, Vertex { pos: left_positions[3], uv: [uv_storage.left_east[2], uv_storage.left_east[3]] }, Vertex { pos: left_positions[2], uv: [uv_storage.left_east[0], uv_storage.left_east[3]] }]),
        Quad([Vertex { pos: left_positions[5], uv: [uv_storage.left_south[0], uv_storage.left_south[1]] }, Vertex { pos: left_positions[0], uv: [uv_storage.left_south[2], uv_storage.left_south[1]] }, Vertex { pos: left_positions[2], uv: [uv_storage.left_south[2], uv_storage.left_south[3]] }, Vertex { pos: left_positions[7], uv: [uv_storage.left_south[0], uv_storage.left_south[3]] }]),
        Quad([Vertex { pos: left_positions[4], uv: [uv_storage.left_west[0], uv_storage.left_west[1]] }, Vertex { pos: left_positions[5], uv: [uv_storage.left_west[2], uv_storage.left_west[1]] }, Vertex { pos: left_positions[7], uv: [uv_storage.left_west[2], uv_storage.left_west[3]] }, Vertex { pos: left_positions[6], uv: [uv_storage.left_west[0], uv_storage.left_west[3]] }]),
        Quad([Vertex { pos: left_positions[4], uv: [uv_storage.left_up[2], uv_storage.left_up[1]] }, Vertex { pos: left_positions[1], uv: [uv_storage.left_up[2], uv_storage.left_up[3]] }, Vertex { pos: left_positions[0], uv: [uv_storage.left_up[0], uv_storage.left_up[3]] }, Vertex { pos: left_positions[5], uv: [uv_storage.left_up[0], uv_storage.left_up[1]] }]),
        Quad([Vertex { pos: left_positions[7], uv: [uv_storage.left_down[2], uv_storage.left_down[1]] }, Vertex { pos: left_positions[2], uv: [uv_storage.left_down[2], uv_storage.left_down[3]] }, Vertex { pos: left_positions[3], uv: [uv_storage.left_down[0], uv_storage.left_down[3]] }, Vertex { pos: left_positions[6], uv: [uv_storage.left_down[0], uv_storage.left_down[1]] }]),
        Quad([Vertex { pos: right_positions[1], uv: [uv_storage.right_north[0], uv_storage.right_north[1]] }, Vertex { pos: right_positions[4], uv: [uv_storage.right_north[2], uv_storage.right_north[1]] }, Vertex { pos: right_positions[6], uv: [uv_storage.right_north[2], uv_storage.right_north[3]] }, Vertex { pos: right_positions[3], uv: [uv_storage.right_north[0], uv_storage.right_north[3]] }]),
        Quad([Vertex { pos: right_positions[0], uv: [uv_storage.right_east[0], uv_storage.right_east[1]] }, Vertex { pos: right_positions[1], uv: [uv_storage.right_east[2], uv_storage.right_east[1]] }, Vertex { pos: right_positions[3], uv: [uv_storage.right_east[2], uv_storage.right_east[3]] }, Vertex { pos: right_positions[2], uv: [uv_storage.right_east[0], uv_storage.right_east[3]] }]),
        Quad([Vertex { pos: right_positions[5], uv: [uv_storage.right_south[0], uv_storage.right_south[1]] }, Vertex { pos: right_positions[0], uv: [uv_storage.right_south[2], uv_storage.right_south[1]] }, Vertex { pos: right_positions[2], uv: [uv_storage.right_south[2], uv_storage.right_south[3]] }, Vertex { pos: right_positions[7], uv: [uv_storage.right_south[0], uv_storage.right_south[3]] }]),
        Quad([Vertex { pos: right_positions[4], uv: [uv_storage.right_west[0], uv_storage.right_west[1]] }, Vertex { pos: right_positions[5], uv: [uv_storage.right_west[2], uv_storage.right_west[1]] }, Vertex { pos: right_positions[7], uv: [uv_storage.right_west[2], uv_storage.right_west[3]] }, Vertex { pos: right_positions[6], uv: [uv_storage.right_west[0], uv_storage.right_west[3]] }]),
        Quad([Vertex { pos: right_positions[4], uv: [uv_storage.right_up[0], uv_storage.right_up[3]] }, Vertex { pos: right_positions[1], uv: [uv_storage.right_up[0], uv_storage.right_up[1]] }, Vertex { pos: right_positions[0], uv: [uv_storage.right_up[2], uv_storage.right_up[1]] }, Vertex { pos: right_positions[5], uv: [uv_storage.right_up[2], uv_storage.right_up[3]] }]),
        Quad([Vertex { pos: right_positions[7], uv: [uv_storage.right_down[2], uv_storage.right_down[1]] }, Vertex { pos: right_positions[2], uv: [uv_storage.right_down[2], uv_storage.right_down[3]] }, Vertex { pos: right_positions[3], uv: [uv_storage.right_down[0], uv_storage.right_down[3]] }, Vertex { pos: right_positions[6], uv: [uv_storage.right_down[0], uv_storage.right_down[1]] }]),
        Quad([Vertex { pos: front_positions[1], uv: [uv_storage.front_north[0], uv_storage.front_north[1]] }, Vertex { pos: front_positions[4], uv: [uv_storage.front_north[2], uv_storage.front_north[1]] }, Vertex { pos: front_positions[6], uv: [uv_storage.front_north[2], uv_storage.front_north[3]] }, Vertex { pos: front_positions[3], uv: [uv_storage.front_north[0], uv_storage.front_north[3]] }]),
        Quad([Vertex { pos: front_positions[0], uv: [uv_storage.front_east[0], uv_storage.front_east[1]] }, Vertex { pos: front_positions[1], uv: [uv_storage.front_east[2], uv_storage.front_east[1]] }, Vertex { pos: front_positions[3], uv: [uv_storage.front_east[2], uv_storage.front_east[3]] }, Vertex { pos: front_positions[2], uv: [uv_storage.front_east[0], uv_storage.front_east[3]] }]),
        Quad([Vertex { pos: front_positions[5], uv: [uv_storage.front_south[0], uv_storage.front_south[1]] }, Vertex { pos: front_positions[0], uv: [uv_storage.front_south[2], uv_storage.front_south[1]] }, Vertex { pos: front_positions[2], uv: [uv_storage.front_south[2], uv_storage.front_south[3]] }, Vertex { pos: front_positions[7], uv: [uv_storage.front_south[0], uv_storage.front_south[3]] }]),
        Quad([Vertex { pos: front_positions[4], uv: [uv_storage.front_west[0], uv_storage.front_west[1]] }, Vertex { pos: front_positions[5], uv: [uv_storage.front_west[2], uv_storage.front_west[1]] }, Vertex { pos: front_positions[7], uv: [uv_storage.front_west[2], uv_storage.front_west[3]] }, Vertex { pos: front_positions[6], uv: [uv_storage.front_west[0], uv_storage.front_west[3]] }]),
        Quad([Vertex { pos: front_positions[4], uv: [uv_storage.front_up[0], uv_storage.front_up[1]] }, Vertex { pos: front_positions[1], uv: [uv_storage.front_up[2], uv_storage.front_up[1]] }, Vertex { pos: front_positions[0], uv: [uv_storage.front_up[2], uv_storage.front_up[3]] }, Vertex { pos: front_positions[5], uv: [uv_storage.front_up[0], uv_storage.front_up[3]] }]),
        Quad([Vertex { pos: front_positions[7], uv: [uv_storage.front_down[0], uv_storage.front_down[1]] }, Vertex { pos: front_positions[2], uv: [uv_storage.front_down[2], uv_storage.front_down[1]] }, Vertex { pos: front_positions[3], uv: [uv_storage.front_down[2], uv_storage.front_down[3]] }, Vertex { pos: front_positions[6], uv: [uv_storage.front_down[0], uv_storage.front_down[3]] }]),
    ]);
}