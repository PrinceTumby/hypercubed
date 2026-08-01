use super::{RenderEntity, RenderEntitySpawnFunc};
use crate::protocol::play::SpawnEntityInfo;
use nalgebra::Point3;
use resources::{Identifier, identifier};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoatVariant {
    Acacia,
    BambooRaft,
    Birch,
    Cherry,
    DarkOak,
    Jungle,
    Mangrove,
    Oak,
    PaleOak,
    Spruce,
}

impl BoatVariant {
    pub fn texture_identifier(self) -> Identifier {
        match self {
            Self::Acacia => identifier!("entity/boat/acacia"),
            Self::BambooRaft => identifier!("entity/boat/bamboo"),
            Self::Birch => identifier!("entity/boat/birch"),
            Self::Cherry => identifier!("entity/boat/cherry"),
            Self::DarkOak => identifier!("entity/boat/dark_oak"),
            Self::Jungle => identifier!("entity/boat/jungle"),
            Self::Mangrove => identifier!("entity/boat/mangrove"),
            Self::Oak => identifier!("entity/boat/oak"),
            Self::PaleOak => identifier!("entity/boat/pale_oak"),
            Self::Spruce => identifier!("entity/boat/spruce"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Boat {
    pub pos: Point3<f32>,
    pub yaw: f32,
    pub variant: BoatVariant,
    pub has_chest: bool,
}

impl Boat {
    pub fn new(variant: BoatVariant, has_chest: bool, info: SpawnEntityInfo) -> Box<Self> {
        Box::new(Self {
            pos: info.coords.map(|n| n as f32).into(),
            yaw: info.yaw.into(),
            variant,
            has_chest,
        })
    }
}

impl RenderEntity for Boat {}

pub static ENTITY_SPAWN_FUNCS: &[(&str, RenderEntitySpawnFunc)] = &[
    ("minecraft:acacia_boat", |info| {
        Boat::new(BoatVariant::Acacia, false, info)
    }),
    ("minecraft:acacia_chest_boat", |info| {
        Boat::new(BoatVariant::Acacia, true, info)
    }),
    ("minecraft:bamboo_raft", |info| {
        Boat::new(BoatVariant::BambooRaft, false, info)
    }),
    ("minecraft:bamboo_chest_raft", |info| {
        Boat::new(BoatVariant::BambooRaft, true, info)
    }),
    ("minecraft:birch_boat", |info| {
        Boat::new(BoatVariant::Birch, false, info)
    }),
    ("minecraft:birch_chest_boat", |info| {
        Boat::new(BoatVariant::Birch, true, info)
    }),
    ("minecraft:cherry_boat", |info| {
        Boat::new(BoatVariant::Cherry, false, info)
    }),
    ("minecraft:cherry_chest_boat", |info| {
        Boat::new(BoatVariant::Cherry, true, info)
    }),
    ("minecraft:dark_oak_boat", |info| {
        Boat::new(BoatVariant::DarkOak, false, info)
    }),
    ("minecraft:dark_oak_chest_boat", |info| {
        Boat::new(BoatVariant::DarkOak, true, info)
    }),
    ("minecraft:jungle_boat", |info| {
        Boat::new(BoatVariant::Jungle, false, info)
    }),
    ("minecraft:jungle_chest_boat", |info| {
        Boat::new(BoatVariant::Jungle, true, info)
    }),
    ("minecraft:mangrove_boat", |info| {
        Boat::new(BoatVariant::Mangrove, false, info)
    }),
    ("minecraft:mangrove_chest_boat", |info| {
        Boat::new(BoatVariant::Mangrove, true, info)
    }),
    ("minecraft:oak_boat", |info| {
        Boat::new(BoatVariant::Oak, false, info)
    }),
    ("minecraft:oak_chest_boat", |info| {
        Boat::new(BoatVariant::Oak, true, info)
    }),
    ("minecraft:pale_oak_boat", |info| {
        Boat::new(BoatVariant::PaleOak, false, info)
    }),
    ("minecraft:pale_oak_chest_boat", |info| {
        Boat::new(BoatVariant::PaleOak, true, info)
    }),
    ("minecraft:spruce_boat", |info| {
        Boat::new(BoatVariant::Spruce, false, info)
    }),
    ("minecraft:spruce_chest_boat", |info| {
        Boat::new(BoatVariant::Spruce, true, info)
    }),
];
