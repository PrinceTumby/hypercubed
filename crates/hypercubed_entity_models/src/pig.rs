// This file was generated from "pig.bbmodel" using the Hypercubed Blockbench plugin.

#![allow(unused)]

use anyhow::Context as _;
use hypercubed_core::types::PercentageF32;
use nalgebra::{Matrix4, Point3, Vector3};
use resources::Identifier;

use super::{EntityRenderQuad as Quad, EntityRenderVertex as Vertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UvStorage {
    pub body_north: [u16; 4],
    pub body_east: [u16; 4],
    pub body_south: [u16; 4],
    pub body_west: [u16; 4],
    pub body_up: [u16; 4],
    pub body_down: [u16; 4],
    pub leg_back_left_north: [u16; 4],
    pub leg_back_left_east: [u16; 4],
    pub leg_back_left_south: [u16; 4],
    pub leg_back_left_west: [u16; 4],
    pub leg_back_left_up: [u16; 4],
    pub leg_back_left_down: [u16; 4],
    pub head_north: [u16; 4],
    pub head_east: [u16; 4],
    pub head_south: [u16; 4],
    pub head_west: [u16; 4],
    pub head_up: [u16; 4],
    pub head_down: [u16; 4],
    pub snout_east: [u16; 4],
    pub snout_south: [u16; 4],
    pub snout_west: [u16; 4],
    pub snout_up: [u16; 4],
    pub snout_down: [u16; 4],
    pub leg_back_right_north: [u16; 4],
    pub leg_back_right_east: [u16; 4],
    pub leg_back_right_south: [u16; 4],
    pub leg_back_right_west: [u16; 4],
    pub leg_back_right_up: [u16; 4],
    pub leg_back_right_down: [u16; 4],
    pub leg_front_left_north: [u16; 4],
    pub leg_front_left_east: [u16; 4],
    pub leg_front_left_south: [u16; 4],
    pub leg_front_left_west: [u16; 4],
    pub leg_front_left_up: [u16; 4],
    pub leg_front_left_down: [u16; 4],
    pub leg_front_right_north: [u16; 4],
    pub leg_front_right_east: [u16; 4],
    pub leg_front_right_south: [u16; 4],
    pub leg_front_right_west: [u16; 4],
    pub leg_front_right_up: [u16; 4],
    pub leg_front_right_down: [u16; 4],
}

impl UvStorage {
    pub fn load_from(atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        Ok(Self {
            body_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/north").unwrap())
                .context("Error while loading texture part \"body/north\"")?
                .basic_or_first_frame_uvs(),
            body_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/east").unwrap())
                .context("Error while loading texture part \"body/east\"")?
                .basic_or_first_frame_uvs(),
            body_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/south").unwrap())
                .context("Error while loading texture part \"body/south\"")?
                .basic_or_first_frame_uvs(),
            body_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/west").unwrap())
                .context("Error while loading texture part \"body/west\"")?
                .basic_or_first_frame_uvs(),
            body_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/up").unwrap())
                .context("Error while loading texture part \"body/up\"")?
                .basic_or_first_frame_uvs(),
            body_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/body/down").unwrap())
                .context("Error while loading texture part \"body/down\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/north").unwrap())
                .context("Error while loading texture part \"leg_back_left/north\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/east").unwrap())
                .context("Error while loading texture part \"leg_back_left/east\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/south").unwrap())
                .context("Error while loading texture part \"leg_back_left/south\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/west").unwrap())
                .context("Error while loading texture part \"leg_back_left/west\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/up").unwrap())
                .context("Error while loading texture part \"leg_back_left/up\"")?
                .basic_or_first_frame_uvs(),
            leg_back_left_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/down").unwrap())
                .context("Error while loading texture part \"leg_back_left/down\"")?
                .basic_or_first_frame_uvs(),
            head_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/north").unwrap())
                .context("Error while loading texture part \"head/north\"")?
                .basic_or_first_frame_uvs(),
            head_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/east").unwrap())
                .context("Error while loading texture part \"head/east\"")?
                .basic_or_first_frame_uvs(),
            head_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/south").unwrap())
                .context("Error while loading texture part \"head/south\"")?
                .basic_or_first_frame_uvs(),
            head_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/west").unwrap())
                .context("Error while loading texture part \"head/west\"")?
                .basic_or_first_frame_uvs(),
            head_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/up").unwrap())
                .context("Error while loading texture part \"head/up\"")?
                .basic_or_first_frame_uvs(),
            head_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/head/down").unwrap())
                .context("Error while loading texture part \"head/down\"")?
                .basic_or_first_frame_uvs(),
            snout_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/snout/east").unwrap())
                .context("Error while loading texture part \"snout/east\"")?
                .basic_or_first_frame_uvs(),
            snout_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/snout/south").unwrap())
                .context("Error while loading texture part \"snout/south\"")?
                .basic_or_first_frame_uvs(),
            snout_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/snout/west").unwrap())
                .context("Error while loading texture part \"snout/west\"")?
                .basic_or_first_frame_uvs(),
            snout_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/snout/up").unwrap())
                .context("Error while loading texture part \"snout/up\"")?
                .basic_or_first_frame_uvs(),
            snout_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/snout/down").unwrap())
                .context("Error while loading texture part \"snout/down\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/north").unwrap())
                .context("Error while loading texture part \"leg_back_right/north\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/east").unwrap())
                .context("Error while loading texture part \"leg_back_right/east\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/south").unwrap())
                .context("Error while loading texture part \"leg_back_right/south\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/west").unwrap())
                .context("Error while loading texture part \"leg_back_right/west\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/up").unwrap())
                .context("Error while loading texture part \"leg_back_right/up\"")?
                .basic_or_first_frame_uvs(),
            leg_back_right_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/down").unwrap())
                .context("Error while loading texture part \"leg_back_right/down\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/north").unwrap())
                .context("Error while loading texture part \"leg_front_left/north\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/east").unwrap())
                .context("Error while loading texture part \"leg_front_left/east\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/south").unwrap())
                .context("Error while loading texture part \"leg_front_left/south\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/west").unwrap())
                .context("Error while loading texture part \"leg_front_left/west\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/up").unwrap())
                .context("Error while loading texture part \"leg_front_left/up\"")?
                .basic_or_first_frame_uvs(),
            leg_front_left_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/down").unwrap())
                .context("Error while loading texture part \"leg_front_left/down\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_north: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/north").unwrap())
                .context("Error while loading texture part \"leg_front_right/north\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_east: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/east").unwrap())
                .context("Error while loading texture part \"leg_front_right/east\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_south: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/south").unwrap())
                .context("Error while loading texture part \"leg_front_right/south\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_west: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/west").unwrap())
                .context("Error while loading texture part \"leg_front_right/west\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_up: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/up").unwrap())
                .context("Error while loading texture part \"leg_front_right/up\"")?
                .basic_or_first_frame_uvs(),
            leg_front_right_down: atlas
                .get_texture(&Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/down").unwrap())
                .context("Error while loading texture part \"leg_front_right/down\"")?
                .basic_or_first_frame_uvs(),
        })
    }
}

#[cfg(feature = "std")]
pub fn load_textures(atlas_builder: &mut resources::texture::AtlasBuilder) -> anyhow::Result<()> {
    atlas_builder.load_texture_parts(
        &Identifier::parse("minecraft:entity/pig/pig").unwrap(),
        [
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.71875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.71875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.84375_f32),
                    PercentageF32::from_f32_0_1_clamp(1_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.5625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.71875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.4375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5625_f32),
                    PercentageF32::from_f32_0_1_clamp(1_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.84375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(1_f32),
                    PercentageF32::from_f32_0_1_clamp(1_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/body/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.5625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.71875_f32),
                    PercentageF32::from_f32_0_1_clamp(1_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_left/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/head/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/snout/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.53125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.34375_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/snout/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.53125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/snout/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.53125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/snout/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.265625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.53125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/snout/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.328125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.390625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.53125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_back_right/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_left/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/north").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.25_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/east").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/south").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/west").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.8125_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/up").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.0625_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
                ],
            ),
            (
                Identifier::parse("hypercubed_vanilla:entity/pig/leg_front_right/down").unwrap(),
                [
                    PercentageF32::from_f32_0_1_clamp(0.125_f32),
                    PercentageF32::from_f32_0_1_clamp(0.5_f32),
                    PercentageF32::from_f32_0_1_clamp(0.1875_f32),
                    PercentageF32::from_f32_0_1_clamp(0.625_f32),
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
    const BODY_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.3125_f32, 0.875_f32, 0.5_f32),
        Point3::new(0.3125_f32, 0.875_f32, -0.5_f32),
        Point3::new(0.3125_f32, 0.375_f32, 0.5_f32),
        Point3::new(0.3125_f32, 0.375_f32, -0.5_f32),
        Point3::new(-0.3125_f32, 0.875_f32, -0.5_f32),
        Point3::new(-0.3125_f32, 0.875_f32, 0.5_f32),
        Point3::new(-0.3125_f32, 0.375_f32, -0.5_f32),
        Point3::new(-0.3125_f32, 0.375_f32, 0.5_f32),
    ];
    let body_positions: [[f32; 3]; 8] =
        BODY_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const LEG_BACK_LEFT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.3125_f32, 0.375_f32, -0.3125_f32),
        Point3::new(0.3125_f32, 0.375_f32, -0.5625_f32),
        Point3::new(0.3125_f32, 0_f32, -0.3125_f32),
        Point3::new(0.3125_f32, 0_f32, -0.5625_f32),
        Point3::new(0.0625_f32, 0.375_f32, -0.5625_f32),
        Point3::new(0.0625_f32, 0.375_f32, -0.3125_f32),
        Point3::new(0.0625_f32, 0_f32, -0.5625_f32),
        Point3::new(0.0625_f32, 0_f32, -0.3125_f32),
    ];
    let leg_back_left_positions: [[f32; 3]; 8] =
        LEG_BACK_LEFT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const HEAD_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.25_f32, 1_f32, 0.875_f32),
        Point3::new(0.25_f32, 1_f32, 0.375_f32),
        Point3::new(0.25_f32, 0.5_f32, 0.875_f32),
        Point3::new(0.25_f32, 0.5_f32, 0.375_f32),
        Point3::new(-0.25_f32, 1_f32, 0.375_f32),
        Point3::new(-0.25_f32, 1_f32, 0.875_f32),
        Point3::new(-0.25_f32, 0.5_f32, 0.375_f32),
        Point3::new(-0.25_f32, 0.5_f32, 0.875_f32),
    ];
    let head_positions: [[f32; 3]; 8] =
        HEAD_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const SNOUT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.125_f32, 0.75_f32, 0.9375_f32),
        Point3::new(0.125_f32, 0.75_f32, 0.875_f32),
        Point3::new(0.125_f32, 0.5625_f32, 0.9375_f32),
        Point3::new(0.125_f32, 0.5625_f32, 0.875_f32),
        Point3::new(-0.125_f32, 0.75_f32, 0.875_f32),
        Point3::new(-0.125_f32, 0.75_f32, 0.9375_f32),
        Point3::new(-0.125_f32, 0.5625_f32, 0.875_f32),
        Point3::new(-0.125_f32, 0.5625_f32, 0.9375_f32),
    ];
    let snout_positions: [[f32; 3]; 8] =
        SNOUT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const LEG_BACK_RIGHT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(-0.0625_f32, 0.375_f32, -0.3125_f32),
        Point3::new(-0.0625_f32, 0.375_f32, -0.5625_f32),
        Point3::new(-0.0625_f32, 0_f32, -0.3125_f32),
        Point3::new(-0.0625_f32, 0_f32, -0.5625_f32),
        Point3::new(-0.3125_f32, 0.375_f32, -0.5625_f32),
        Point3::new(-0.3125_f32, 0.375_f32, -0.3125_f32),
        Point3::new(-0.3125_f32, 0_f32, -0.5625_f32),
        Point3::new(-0.3125_f32, 0_f32, -0.3125_f32),
    ];
    let leg_back_right_positions: [[f32; 3]; 8] =
        LEG_BACK_RIGHT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const LEG_FRONT_LEFT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(0.3125_f32, 0.375_f32, 0.4375_f32),
        Point3::new(0.3125_f32, 0.375_f32, 0.1875_f32),
        Point3::new(0.3125_f32, 0_f32, 0.4375_f32),
        Point3::new(0.3125_f32, 0_f32, 0.1875_f32),
        Point3::new(0.0625_f32, 0.375_f32, 0.1875_f32),
        Point3::new(0.0625_f32, 0.375_f32, 0.4375_f32),
        Point3::new(0.0625_f32, 0_f32, 0.1875_f32),
        Point3::new(0.0625_f32, 0_f32, 0.4375_f32),
    ];
    let leg_front_left_positions: [[f32; 3]; 8] =
        LEG_FRONT_LEFT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    const LEG_FRONT_RIGHT_BASE_POSITIONS: [Point3<f32>; 8] = [
        Point3::new(-0.0625_f32, 0.375_f32, 0.4375_f32),
        Point3::new(-0.0625_f32, 0.375_f32, 0.1875_f32),
        Point3::new(-0.0625_f32, 0_f32, 0.4375_f32),
        Point3::new(-0.0625_f32, 0_f32, 0.1875_f32),
        Point3::new(-0.3125_f32, 0.375_f32, 0.1875_f32),
        Point3::new(-0.3125_f32, 0.375_f32, 0.4375_f32),
        Point3::new(-0.3125_f32, 0_f32, 0.1875_f32),
        Point3::new(-0.3125_f32, 0_f32, 0.4375_f32),
    ];
    let leg_front_right_positions: [[f32; 3]; 8] =
        LEG_FRONT_RIGHT_BASE_POSITIONS.map(|p| matrix.transform_point(&p).into());
    out_quads.extend([
        Quad([Vertex { pos: body_positions[1], uv: [uv_storage.body_north[2], uv_storage.body_north[1]] }, Vertex { pos: body_positions[4], uv: [uv_storage.body_north[0], uv_storage.body_north[1]] }, Vertex { pos: body_positions[6], uv: [uv_storage.body_north[0], uv_storage.body_north[3]] }, Vertex { pos: body_positions[3], uv: [uv_storage.body_north[2], uv_storage.body_north[3]] }]),
        Quad([Vertex { pos: body_positions[0], uv: [uv_storage.body_east[2], uv_storage.body_east[1]] }, Vertex { pos: body_positions[1], uv: [uv_storage.body_east[2], uv_storage.body_east[3]] }, Vertex { pos: body_positions[3], uv: [uv_storage.body_east[0], uv_storage.body_east[3]] }, Vertex { pos: body_positions[2], uv: [uv_storage.body_east[0], uv_storage.body_east[1]] }]),
        Quad([Vertex { pos: body_positions[5], uv: [uv_storage.body_south[0], uv_storage.body_south[1]] }, Vertex { pos: body_positions[0], uv: [uv_storage.body_south[2], uv_storage.body_south[1]] }, Vertex { pos: body_positions[2], uv: [uv_storage.body_south[2], uv_storage.body_south[3]] }, Vertex { pos: body_positions[7], uv: [uv_storage.body_south[0], uv_storage.body_south[3]] }]),
        Quad([Vertex { pos: body_positions[4], uv: [uv_storage.body_west[0], uv_storage.body_west[3]] }, Vertex { pos: body_positions[5], uv: [uv_storage.body_west[0], uv_storage.body_west[1]] }, Vertex { pos: body_positions[7], uv: [uv_storage.body_west[2], uv_storage.body_west[1]] }, Vertex { pos: body_positions[6], uv: [uv_storage.body_west[2], uv_storage.body_west[3]] }]),
        Quad([Vertex { pos: body_positions[4], uv: [uv_storage.body_up[2], uv_storage.body_up[3]] }, Vertex { pos: body_positions[1], uv: [uv_storage.body_up[0], uv_storage.body_up[3]] }, Vertex { pos: body_positions[0], uv: [uv_storage.body_up[0], uv_storage.body_up[1]] }, Vertex { pos: body_positions[5], uv: [uv_storage.body_up[2], uv_storage.body_up[1]] }]),
        Quad([Vertex { pos: body_positions[7], uv: [uv_storage.body_down[0], uv_storage.body_down[1]] }, Vertex { pos: body_positions[2], uv: [uv_storage.body_down[2], uv_storage.body_down[1]] }, Vertex { pos: body_positions[3], uv: [uv_storage.body_down[2], uv_storage.body_down[3]] }, Vertex { pos: body_positions[6], uv: [uv_storage.body_down[0], uv_storage.body_down[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[1], uv: [uv_storage.leg_back_left_north[0], uv_storage.leg_back_left_north[1]] }, Vertex { pos: leg_back_left_positions[4], uv: [uv_storage.leg_back_left_north[2], uv_storage.leg_back_left_north[1]] }, Vertex { pos: leg_back_left_positions[6], uv: [uv_storage.leg_back_left_north[2], uv_storage.leg_back_left_north[3]] }, Vertex { pos: leg_back_left_positions[3], uv: [uv_storage.leg_back_left_north[0], uv_storage.leg_back_left_north[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[0], uv: [uv_storage.leg_back_left_east[0], uv_storage.leg_back_left_east[1]] }, Vertex { pos: leg_back_left_positions[1], uv: [uv_storage.leg_back_left_east[2], uv_storage.leg_back_left_east[1]] }, Vertex { pos: leg_back_left_positions[3], uv: [uv_storage.leg_back_left_east[2], uv_storage.leg_back_left_east[3]] }, Vertex { pos: leg_back_left_positions[2], uv: [uv_storage.leg_back_left_east[0], uv_storage.leg_back_left_east[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[5], uv: [uv_storage.leg_back_left_south[0], uv_storage.leg_back_left_south[1]] }, Vertex { pos: leg_back_left_positions[0], uv: [uv_storage.leg_back_left_south[2], uv_storage.leg_back_left_south[1]] }, Vertex { pos: leg_back_left_positions[2], uv: [uv_storage.leg_back_left_south[2], uv_storage.leg_back_left_south[3]] }, Vertex { pos: leg_back_left_positions[7], uv: [uv_storage.leg_back_left_south[0], uv_storage.leg_back_left_south[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[4], uv: [uv_storage.leg_back_left_west[0], uv_storage.leg_back_left_west[1]] }, Vertex { pos: leg_back_left_positions[5], uv: [uv_storage.leg_back_left_west[2], uv_storage.leg_back_left_west[1]] }, Vertex { pos: leg_back_left_positions[7], uv: [uv_storage.leg_back_left_west[2], uv_storage.leg_back_left_west[3]] }, Vertex { pos: leg_back_left_positions[6], uv: [uv_storage.leg_back_left_west[0], uv_storage.leg_back_left_west[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[4], uv: [uv_storage.leg_back_left_up[0], uv_storage.leg_back_left_up[1]] }, Vertex { pos: leg_back_left_positions[1], uv: [uv_storage.leg_back_left_up[2], uv_storage.leg_back_left_up[1]] }, Vertex { pos: leg_back_left_positions[0], uv: [uv_storage.leg_back_left_up[2], uv_storage.leg_back_left_up[3]] }, Vertex { pos: leg_back_left_positions[5], uv: [uv_storage.leg_back_left_up[0], uv_storage.leg_back_left_up[3]] }]),
        Quad([Vertex { pos: leg_back_left_positions[7], uv: [uv_storage.leg_back_left_down[0], uv_storage.leg_back_left_down[1]] }, Vertex { pos: leg_back_left_positions[2], uv: [uv_storage.leg_back_left_down[2], uv_storage.leg_back_left_down[1]] }, Vertex { pos: leg_back_left_positions[3], uv: [uv_storage.leg_back_left_down[2], uv_storage.leg_back_left_down[3]] }, Vertex { pos: leg_back_left_positions[6], uv: [uv_storage.leg_back_left_down[0], uv_storage.leg_back_left_down[3]] }]),
        Quad([Vertex { pos: head_positions[1], uv: [uv_storage.head_north[0], uv_storage.head_north[1]] }, Vertex { pos: head_positions[4], uv: [uv_storage.head_north[2], uv_storage.head_north[1]] }, Vertex { pos: head_positions[6], uv: [uv_storage.head_north[2], uv_storage.head_north[3]] }, Vertex { pos: head_positions[3], uv: [uv_storage.head_north[0], uv_storage.head_north[3]] }]),
        Quad([Vertex { pos: head_positions[0], uv: [uv_storage.head_east[0], uv_storage.head_east[1]] }, Vertex { pos: head_positions[1], uv: [uv_storage.head_east[2], uv_storage.head_east[1]] }, Vertex { pos: head_positions[3], uv: [uv_storage.head_east[2], uv_storage.head_east[3]] }, Vertex { pos: head_positions[2], uv: [uv_storage.head_east[0], uv_storage.head_east[3]] }]),
        Quad([Vertex { pos: head_positions[5], uv: [uv_storage.head_south[0], uv_storage.head_south[1]] }, Vertex { pos: head_positions[0], uv: [uv_storage.head_south[2], uv_storage.head_south[1]] }, Vertex { pos: head_positions[2], uv: [uv_storage.head_south[2], uv_storage.head_south[3]] }, Vertex { pos: head_positions[7], uv: [uv_storage.head_south[0], uv_storage.head_south[3]] }]),
        Quad([Vertex { pos: head_positions[4], uv: [uv_storage.head_west[0], uv_storage.head_west[1]] }, Vertex { pos: head_positions[5], uv: [uv_storage.head_west[2], uv_storage.head_west[1]] }, Vertex { pos: head_positions[7], uv: [uv_storage.head_west[2], uv_storage.head_west[3]] }, Vertex { pos: head_positions[6], uv: [uv_storage.head_west[0], uv_storage.head_west[3]] }]),
        Quad([Vertex { pos: head_positions[4], uv: [uv_storage.head_up[0], uv_storage.head_up[1]] }, Vertex { pos: head_positions[1], uv: [uv_storage.head_up[2], uv_storage.head_up[1]] }, Vertex { pos: head_positions[0], uv: [uv_storage.head_up[2], uv_storage.head_up[3]] }, Vertex { pos: head_positions[5], uv: [uv_storage.head_up[0], uv_storage.head_up[3]] }]),
        Quad([Vertex { pos: head_positions[7], uv: [uv_storage.head_down[0], uv_storage.head_down[1]] }, Vertex { pos: head_positions[2], uv: [uv_storage.head_down[2], uv_storage.head_down[1]] }, Vertex { pos: head_positions[3], uv: [uv_storage.head_down[2], uv_storage.head_down[3]] }, Vertex { pos: head_positions[6], uv: [uv_storage.head_down[0], uv_storage.head_down[3]] }]),
        Quad([Vertex { pos: snout_positions[0], uv: [uv_storage.snout_east[0], uv_storage.snout_east[1]] }, Vertex { pos: snout_positions[1], uv: [uv_storage.snout_east[2], uv_storage.snout_east[1]] }, Vertex { pos: snout_positions[3], uv: [uv_storage.snout_east[2], uv_storage.snout_east[3]] }, Vertex { pos: snout_positions[2], uv: [uv_storage.snout_east[0], uv_storage.snout_east[3]] }]),
        Quad([Vertex { pos: snout_positions[5], uv: [uv_storage.snout_south[0], uv_storage.snout_south[1]] }, Vertex { pos: snout_positions[0], uv: [uv_storage.snout_south[2], uv_storage.snout_south[1]] }, Vertex { pos: snout_positions[2], uv: [uv_storage.snout_south[2], uv_storage.snout_south[3]] }, Vertex { pos: snout_positions[7], uv: [uv_storage.snout_south[0], uv_storage.snout_south[3]] }]),
        Quad([Vertex { pos: snout_positions[4], uv: [uv_storage.snout_west[0], uv_storage.snout_west[1]] }, Vertex { pos: snout_positions[5], uv: [uv_storage.snout_west[2], uv_storage.snout_west[1]] }, Vertex { pos: snout_positions[7], uv: [uv_storage.snout_west[2], uv_storage.snout_west[3]] }, Vertex { pos: snout_positions[6], uv: [uv_storage.snout_west[0], uv_storage.snout_west[3]] }]),
        Quad([Vertex { pos: snout_positions[4], uv: [uv_storage.snout_up[0], uv_storage.snout_up[1]] }, Vertex { pos: snout_positions[1], uv: [uv_storage.snout_up[2], uv_storage.snout_up[1]] }, Vertex { pos: snout_positions[0], uv: [uv_storage.snout_up[2], uv_storage.snout_up[3]] }, Vertex { pos: snout_positions[5], uv: [uv_storage.snout_up[0], uv_storage.snout_up[3]] }]),
        Quad([Vertex { pos: snout_positions[7], uv: [uv_storage.snout_down[0], uv_storage.snout_down[1]] }, Vertex { pos: snout_positions[2], uv: [uv_storage.snout_down[2], uv_storage.snout_down[1]] }, Vertex { pos: snout_positions[3], uv: [uv_storage.snout_down[2], uv_storage.snout_down[3]] }, Vertex { pos: snout_positions[6], uv: [uv_storage.snout_down[0], uv_storage.snout_down[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[1], uv: [uv_storage.leg_back_right_north[0], uv_storage.leg_back_right_north[1]] }, Vertex { pos: leg_back_right_positions[4], uv: [uv_storage.leg_back_right_north[2], uv_storage.leg_back_right_north[1]] }, Vertex { pos: leg_back_right_positions[6], uv: [uv_storage.leg_back_right_north[2], uv_storage.leg_back_right_north[3]] }, Vertex { pos: leg_back_right_positions[3], uv: [uv_storage.leg_back_right_north[0], uv_storage.leg_back_right_north[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[0], uv: [uv_storage.leg_back_right_east[0], uv_storage.leg_back_right_east[1]] }, Vertex { pos: leg_back_right_positions[1], uv: [uv_storage.leg_back_right_east[2], uv_storage.leg_back_right_east[1]] }, Vertex { pos: leg_back_right_positions[3], uv: [uv_storage.leg_back_right_east[2], uv_storage.leg_back_right_east[3]] }, Vertex { pos: leg_back_right_positions[2], uv: [uv_storage.leg_back_right_east[0], uv_storage.leg_back_right_east[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[5], uv: [uv_storage.leg_back_right_south[0], uv_storage.leg_back_right_south[1]] }, Vertex { pos: leg_back_right_positions[0], uv: [uv_storage.leg_back_right_south[2], uv_storage.leg_back_right_south[1]] }, Vertex { pos: leg_back_right_positions[2], uv: [uv_storage.leg_back_right_south[2], uv_storage.leg_back_right_south[3]] }, Vertex { pos: leg_back_right_positions[7], uv: [uv_storage.leg_back_right_south[0], uv_storage.leg_back_right_south[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[4], uv: [uv_storage.leg_back_right_west[0], uv_storage.leg_back_right_west[1]] }, Vertex { pos: leg_back_right_positions[5], uv: [uv_storage.leg_back_right_west[2], uv_storage.leg_back_right_west[1]] }, Vertex { pos: leg_back_right_positions[7], uv: [uv_storage.leg_back_right_west[2], uv_storage.leg_back_right_west[3]] }, Vertex { pos: leg_back_right_positions[6], uv: [uv_storage.leg_back_right_west[0], uv_storage.leg_back_right_west[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[4], uv: [uv_storage.leg_back_right_up[0], uv_storage.leg_back_right_up[1]] }, Vertex { pos: leg_back_right_positions[1], uv: [uv_storage.leg_back_right_up[2], uv_storage.leg_back_right_up[1]] }, Vertex { pos: leg_back_right_positions[0], uv: [uv_storage.leg_back_right_up[2], uv_storage.leg_back_right_up[3]] }, Vertex { pos: leg_back_right_positions[5], uv: [uv_storage.leg_back_right_up[0], uv_storage.leg_back_right_up[3]] }]),
        Quad([Vertex { pos: leg_back_right_positions[7], uv: [uv_storage.leg_back_right_down[0], uv_storage.leg_back_right_down[1]] }, Vertex { pos: leg_back_right_positions[2], uv: [uv_storage.leg_back_right_down[2], uv_storage.leg_back_right_down[1]] }, Vertex { pos: leg_back_right_positions[3], uv: [uv_storage.leg_back_right_down[2], uv_storage.leg_back_right_down[3]] }, Vertex { pos: leg_back_right_positions[6], uv: [uv_storage.leg_back_right_down[0], uv_storage.leg_back_right_down[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[1], uv: [uv_storage.leg_front_left_north[0], uv_storage.leg_front_left_north[1]] }, Vertex { pos: leg_front_left_positions[4], uv: [uv_storage.leg_front_left_north[2], uv_storage.leg_front_left_north[1]] }, Vertex { pos: leg_front_left_positions[6], uv: [uv_storage.leg_front_left_north[2], uv_storage.leg_front_left_north[3]] }, Vertex { pos: leg_front_left_positions[3], uv: [uv_storage.leg_front_left_north[0], uv_storage.leg_front_left_north[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[0], uv: [uv_storage.leg_front_left_east[0], uv_storage.leg_front_left_east[1]] }, Vertex { pos: leg_front_left_positions[1], uv: [uv_storage.leg_front_left_east[2], uv_storage.leg_front_left_east[1]] }, Vertex { pos: leg_front_left_positions[3], uv: [uv_storage.leg_front_left_east[2], uv_storage.leg_front_left_east[3]] }, Vertex { pos: leg_front_left_positions[2], uv: [uv_storage.leg_front_left_east[0], uv_storage.leg_front_left_east[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[5], uv: [uv_storage.leg_front_left_south[0], uv_storage.leg_front_left_south[1]] }, Vertex { pos: leg_front_left_positions[0], uv: [uv_storage.leg_front_left_south[2], uv_storage.leg_front_left_south[1]] }, Vertex { pos: leg_front_left_positions[2], uv: [uv_storage.leg_front_left_south[2], uv_storage.leg_front_left_south[3]] }, Vertex { pos: leg_front_left_positions[7], uv: [uv_storage.leg_front_left_south[0], uv_storage.leg_front_left_south[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[4], uv: [uv_storage.leg_front_left_west[0], uv_storage.leg_front_left_west[1]] }, Vertex { pos: leg_front_left_positions[5], uv: [uv_storage.leg_front_left_west[2], uv_storage.leg_front_left_west[1]] }, Vertex { pos: leg_front_left_positions[7], uv: [uv_storage.leg_front_left_west[2], uv_storage.leg_front_left_west[3]] }, Vertex { pos: leg_front_left_positions[6], uv: [uv_storage.leg_front_left_west[0], uv_storage.leg_front_left_west[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[4], uv: [uv_storage.leg_front_left_up[0], uv_storage.leg_front_left_up[1]] }, Vertex { pos: leg_front_left_positions[1], uv: [uv_storage.leg_front_left_up[2], uv_storage.leg_front_left_up[1]] }, Vertex { pos: leg_front_left_positions[0], uv: [uv_storage.leg_front_left_up[2], uv_storage.leg_front_left_up[3]] }, Vertex { pos: leg_front_left_positions[5], uv: [uv_storage.leg_front_left_up[0], uv_storage.leg_front_left_up[3]] }]),
        Quad([Vertex { pos: leg_front_left_positions[7], uv: [uv_storage.leg_front_left_down[0], uv_storage.leg_front_left_down[1]] }, Vertex { pos: leg_front_left_positions[2], uv: [uv_storage.leg_front_left_down[2], uv_storage.leg_front_left_down[1]] }, Vertex { pos: leg_front_left_positions[3], uv: [uv_storage.leg_front_left_down[2], uv_storage.leg_front_left_down[3]] }, Vertex { pos: leg_front_left_positions[6], uv: [uv_storage.leg_front_left_down[0], uv_storage.leg_front_left_down[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[1], uv: [uv_storage.leg_front_right_north[0], uv_storage.leg_front_right_north[1]] }, Vertex { pos: leg_front_right_positions[4], uv: [uv_storage.leg_front_right_north[2], uv_storage.leg_front_right_north[1]] }, Vertex { pos: leg_front_right_positions[6], uv: [uv_storage.leg_front_right_north[2], uv_storage.leg_front_right_north[3]] }, Vertex { pos: leg_front_right_positions[3], uv: [uv_storage.leg_front_right_north[0], uv_storage.leg_front_right_north[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[0], uv: [uv_storage.leg_front_right_east[0], uv_storage.leg_front_right_east[1]] }, Vertex { pos: leg_front_right_positions[1], uv: [uv_storage.leg_front_right_east[2], uv_storage.leg_front_right_east[1]] }, Vertex { pos: leg_front_right_positions[3], uv: [uv_storage.leg_front_right_east[2], uv_storage.leg_front_right_east[3]] }, Vertex { pos: leg_front_right_positions[2], uv: [uv_storage.leg_front_right_east[0], uv_storage.leg_front_right_east[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[5], uv: [uv_storage.leg_front_right_south[0], uv_storage.leg_front_right_south[1]] }, Vertex { pos: leg_front_right_positions[0], uv: [uv_storage.leg_front_right_south[2], uv_storage.leg_front_right_south[1]] }, Vertex { pos: leg_front_right_positions[2], uv: [uv_storage.leg_front_right_south[2], uv_storage.leg_front_right_south[3]] }, Vertex { pos: leg_front_right_positions[7], uv: [uv_storage.leg_front_right_south[0], uv_storage.leg_front_right_south[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[4], uv: [uv_storage.leg_front_right_west[0], uv_storage.leg_front_right_west[1]] }, Vertex { pos: leg_front_right_positions[5], uv: [uv_storage.leg_front_right_west[2], uv_storage.leg_front_right_west[1]] }, Vertex { pos: leg_front_right_positions[7], uv: [uv_storage.leg_front_right_west[2], uv_storage.leg_front_right_west[3]] }, Vertex { pos: leg_front_right_positions[6], uv: [uv_storage.leg_front_right_west[0], uv_storage.leg_front_right_west[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[4], uv: [uv_storage.leg_front_right_up[0], uv_storage.leg_front_right_up[1]] }, Vertex { pos: leg_front_right_positions[1], uv: [uv_storage.leg_front_right_up[2], uv_storage.leg_front_right_up[1]] }, Vertex { pos: leg_front_right_positions[0], uv: [uv_storage.leg_front_right_up[2], uv_storage.leg_front_right_up[3]] }, Vertex { pos: leg_front_right_positions[5], uv: [uv_storage.leg_front_right_up[0], uv_storage.leg_front_right_up[3]] }]),
        Quad([Vertex { pos: leg_front_right_positions[7], uv: [uv_storage.leg_front_right_down[0], uv_storage.leg_front_right_down[1]] }, Vertex { pos: leg_front_right_positions[2], uv: [uv_storage.leg_front_right_down[2], uv_storage.leg_front_right_down[1]] }, Vertex { pos: leg_front_right_positions[3], uv: [uv_storage.leg_front_right_down[2], uv_storage.leg_front_right_down[3]] }, Vertex { pos: leg_front_right_positions[6], uv: [uv_storage.leg_front_right_down[0], uv_storage.leg_front_right_down[3]] }]),
    ]);
}