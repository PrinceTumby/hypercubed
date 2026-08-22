use nalgebra::Point3;

use super::blockstate::{
    BlockLightInfo, BlockOpacity, BlockstateInfo, BlockstateInfoModifier, CollisionInfo,
    SkyLightOpacity,
};
use super::{
    BlockstateInfoModifierCase, CustomProperty, FullCustomRegistration, LiquidRegistration,
    Properties, PropertyValue, PropertyValueAtoms, Registration, StandardRegistration,
};
use crate::aabb::AABB;

fn facing_nswe_prop() -> CustomProperty<'static> {
    CustomProperty::enum_variants("facing", vec!["north", "south", "west", "east"])
}

fn facing_neswud_prop() -> CustomProperty<'static> {
    CustomProperty::enum_variants(
        "facing",
        vec!["north", "east", "south", "west", "up", "down"],
    )
}

const WATERLOGGED_PROP: CustomProperty = CustomProperty::boolean("waterlogged");

const POWERED_PROP: CustomProperty = CustomProperty::boolean("powered");

const LIT_PROP: CustomProperty = CustomProperty::boolean("lit");

const STAGE_0_1_PROP: CustomProperty = CustomProperty::int("stage", 0..=1);

const AGE_0_15_PROP: CustomProperty = CustomProperty::int("age", 0..=15);

const AGE_0_25_PROP: CustomProperty = CustomProperty::int("age", 0..=25);

const ROTATION_0_15_PROP: CustomProperty = CustomProperty::int("rotation", 0..=15);

fn chest_type_prop() -> CustomProperty<'static> {
    CustomProperty::enum_variants("type", vec!["single", "left", "right"])
}

const SKY_TRANSPARENT_INFO: BlockLightInfo = BlockLightInfo {
    sky_light_opacity: SkyLightOpacity::Transparent,
    emission_level: 0,
};

fn basic_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration::new(identifier))
}

fn basic_light_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Opaque,
                emission_level,
            },
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn basic_liquid_reg(identifier: &str) -> Registration<'_> {
    Registration::Liquid(LiquidRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Translucent,
                emission_level: 0,
            },
            ..Default::default()
        },
        ..LiquidRegistration::new(identifier)
    })
}

fn basic_liquid_light_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Liquid(LiquidRegistration {
        properties: Properties::default(),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Translucent,
                emission_level,
            },
            ..Default::default()
        },
        ..LiquidRegistration::new(identifier)
    })
}

fn transparent_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn transparent_light_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Transparent,
                emission_level,
            },
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn transparent_no_collider_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn transparent_light_no_collider_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Transparent,
                emission_level,
            },
            collision_info: CollisionInfo::Empty,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn log_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_override: Some(vec![("axis", "y").into()]),
        ..StandardRegistration::new(identifier)
    })
}

fn leaves_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            CustomProperty::int("distance", 1..=7),
            CustomProperty::boolean("persistent"),
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("distance", "7").into(),
            ("persistent", "false").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Leaves,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Translucent,
                emission_level: 0,
            },
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn bed_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_override: Some(vec![
            ("facing", "north").into(),
            ("occupied", "false").into(),
            ("part", "foot").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            collision_info: CollisionInfo::from([AABB {
                corner_1: Point3::new(0.0, 0.0, 0.0),
                corner_2: Point3::new(1.0, 0.5625, 1.0),
            }]),
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn slab_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![WATERLOGGED_PROP]),
        replacement_variants: Some(vec![CustomProperty::enum_variants(
            "type",
            vec!["top", "bottom", "double"],
        )]),
        default_override: Some(vec![
            ("type", "bottom").into(),
            ("waterlogged", "false").into(),
        ]),
        extra_info_modifiers: vec![
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([AABB {
                        corner_1: Point3::new(0.0, 0.0, 0.0),
                        corner_2: Point3::new(1.0, 0.5, 1.0),
                    }])),
                    ..Default::default()
                },
                conditions: vec![("type", "bottom").into()],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([AABB {
                        corner_1: Point3::new(0.0, 0.5, 0.0),
                        corner_2: Point3::new(1.0, 1.0, 1.0),
                    }])),
                    ..Default::default()
                },
                conditions: vec![("type", "top").into()],
            },
        ],
        ..StandardRegistration::new(identifier)
    })
}

fn stairs_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![WATERLOGGED_PROP]),
        replacement_variants: Some(vec![
            facing_nswe_prop(),
            CustomProperty::enum_variants("half", vec!["top", "bottom"]),
            CustomProperty::enum_variants(
                "shape",
                vec![
                    "straight",
                    "inner_left",
                    "inner_right",
                    "outer_left",
                    "outer_right",
                ],
            ),
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("half", "bottom").into(),
            ("shape", "straight").into(),
            ("waterlogged", "false").into(),
            ("type", "bottom").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            ..Default::default()
        },
        extra_info_modifiers: vec![
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).into(),
                        ([0.5, 0.5, 0.0], [1.0, 1.0, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![("shape", "straight").into()],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).into(),
                        ([0.5, 0.5, 0.0], [1.0, 1.0, 1.0]).into(),
                        ([0.0, 0.5, 0.0], [0.5, 1.0, 0.5]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![("shape", "inner_left").into()],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).into(),
                        ([0.5, 0.5, 0.0], [1.0, 1.0, 1.0]).into(),
                        ([0.0, 0.5, 0.5], [0.5, 1.0, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![("shape", "inner_right").into()],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).into(),
                        ([0.5, 0.5, 0.5], [1.0, 1.0, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![("shape", "outer_left").into()],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).into(),
                        ([0.5, 0.5, 0.5], [1.0, 1.0, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![("shape", "outer_right").into()],
            },
        ],
        ..StandardRegistration::new(identifier)
    })
}

fn fence_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            CustomProperty::boolean("east"),
            CustomProperty::boolean("north"),
            CustomProperty::boolean("south"),
            WATERLOGGED_PROP,
            CustomProperty::boolean("west"),
        ]),
        default_override: Some(vec![
            ("east", "false").into(),
            ("north", "false").into(),
            ("south", "false").into(),
            ("waterlogged", "false").into(),
            ("west", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        extra_info_modifiers: vec![
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.375, 0.0, 0.375],
                        [0.625, 1.5, 0.625],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "false").into(),
                    ("south", "false").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.375, 0.0, 0.375],
                        [1.0, 1.5, 0.625],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "false").into(),
                    ("south", "false").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.375, 0.0, 0.0],
                        [0.625, 1.5, 0.625],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "true").into(),
                    ("south", "false").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.375, 0.0, 0.375], [1.0, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 0.375]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "true").into(),
                    ("south", "false").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.375, 0.0, 0.375],
                        [0.625, 1.5, 1.0],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "false").into(),
                    ("south", "true").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.375, 0.0, 0.375], [1.0, 1.5, 1.0]).into(),
                        ([0.375, 0.0, 0.625], [1.0, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "false").into(),
                    ("south", "true").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.375, 0.0, 0.0],
                        [0.625, 1.5, 1.0],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "true").into(),
                    ("south", "true").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.375, 0.0, 0.375], [1.0, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "true").into(),
                    ("south", "true").into(),
                    ("west", "false").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.0, 0.0, 0.375],
                        [0.625, 1.5, 0.625],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "false").into(),
                    ("south", "false").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([(
                        [0.0, 0.0, 0.375],
                        [1.0, 1.5, 0.625],
                    )
                        .into()])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "false").into(),
                    ("south", "false").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [0.375, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 0.625]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "true").into(),
                    ("south", "false").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [1.0, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 0.375]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "true").into(),
                    ("south", "false").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [0.375, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.375], [0.625, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "false").into(),
                    ("south", "true").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [1.0, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.625], [1.0, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "false").into(),
                    ("south", "true").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [0.625, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "false").into(),
                    ("north", "true").into(),
                    ("south", "true").into(),
                    ("west", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    collision_info: Some(CollisionInfo::from([
                        ([0.0, 0.0, 0.375], [1.0, 1.5, 0.625]).into(),
                        ([0.375, 0.0, 0.0], [0.625, 1.5, 1.0]).into(),
                    ])),
                    ..Default::default()
                },
                conditions: vec![
                    ("east", "true").into(),
                    ("north", "true").into(),
                    ("south", "true").into(),
                    ("west", "true").into(),
                ],
            },
        ],
        ..StandardRegistration::new(identifier)
    })
}

fn fence_gate_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             POWERED_PROP,
        ]),
        replacement_variants: Some(vec![
            facing_nswe_prop(),
            CustomProperty::boolean("in_wall"),
            CustomProperty::boolean("open"),
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("in_wall", "false").into(),
            ("open", "false").into(),
            ("powered", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn wall_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            CustomProperty::enum_variants("east", vec!["none", "low", "tall"]),
            CustomProperty::enum_variants("north", vec!["none", "low", "tall"]),
            CustomProperty::enum_variants("south", vec!["none", "low", "tall"]),
            CustomProperty::boolean("up"),
            WATERLOGGED_PROP,
            CustomProperty::enum_variants("west", vec!["none", "low", "tall"]),
        ]),
        default_override: Some(vec![
            ("east", "none").into(),
            ("north", "none").into(),
            ("south", "none").into(),
            ("up", "true").into(),
            ("waterlogged", "false").into(),
            ("west", "none").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn trapdoor_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             POWERED_PROP,
            WATERLOGGED_PROP,
        ]),
        replacement_variants: Some(vec![
            facing_nswe_prop(),
            CustomProperty::enum_variants("half", vec!["top", "bottom"]),
            CustomProperty::boolean("open"),
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("half", "bottom").into(),
            ("open", "false").into(),
            ("powered", "false").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn chest_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             facing_nswe_prop(),
                             chest_type_prop(),
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("type", "single").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn sign_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             ROTATION_0_15_PROP,
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("rotation", "0").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn wall_sign_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             facing_nswe_prop(),
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn hanging_sign_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             CustomProperty::boolean("attached"),
                             ROTATION_0_15_PROP,
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("attached", "false").into(),
            ("rotation", "0").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn wall_hanging_sign_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             facing_nswe_prop(),
            WATERLOGGED_PROP,
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn stained_glass_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn grate_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![WATERLOGGED_PROP]),
        default_override: Some(vec![("waterlogged", "false").into()]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn pressure_plate_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![POWERED_PROP]),
        default_override: Some(vec![("powered", "false").into()]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn weighted_pressure_plate_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        replacement_variants: Some(vec![
            CustomProperty::int("power", 0..=15),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn button_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        replacement_variants: Some(vec![
            CustomProperty::enum_variants("face", vec!["floor", "wall", "ceiling"]),
            facing_nswe_prop(),
            POWERED_PROP,
        ]),
        default_override: Some(vec![
            ("face", "wall").into(),
            ("facing", "north").into(),
            ("powered", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn mushroom_block_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            CustomProperty::boolean("down"),
            CustomProperty::boolean("east"),
            CustomProperty::boolean("north"),
            CustomProperty::boolean("south"),
            CustomProperty::boolean("up"),
            CustomProperty::boolean("west"),
        ]),
        ..StandardRegistration::new(identifier)
    })
}

fn head_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            POWERED_PROP,
            ROTATION_0_15_PROP,
        ]),
        default_override: Some(vec![
            ("powered", "false").into(),
            ("rotation", "0").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn wall_head_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            facing_nswe_prop(),
            POWERED_PROP,
        ]),
        default_override: Some(vec![
            ("facing", "north").into(),
            ("powered", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn glass_pane_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
            CustomProperty::boolean("east"),
            CustomProperty::boolean("north"),
            CustomProperty::boolean("south"),
            WATERLOGGED_PROP,
            CustomProperty::boolean("west"),
        ]),
        default_override: Some(vec![
            ("east", "false").into(),
            ("north", "false").into(),
            ("south", "false").into(),
            ("waterlogged", "false").into(),
            ("west", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn banner_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             ROTATION_0_15_PROP,
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn wall_banner_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             facing_nswe_prop(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn shulker_box_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             facing_neswud_prop(),
        ]),
        default_override: Some(vec![
            ("facing", "up").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn glazed_terracotta_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        replacement_variants: Some(vec![
                             facing_nswe_prop(),
        ]),
        ..StandardRegistration::new(identifier)
    })
}

fn coral_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             WATERLOGGED_PROP,
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn coral_wall_fan_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             WATERLOGGED_PROP,
        ]),
        replacement_variants: Some(vec![
                             facing_nswe_prop(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            collision_info: CollisionInfo::Empty,
        },
        ..StandardRegistration::new(identifier)
    })
}

fn lantern_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             WATERLOGGED_PROP,
        ]),
        replacement_variants: Some(vec![
            CustomProperty::boolean("hanging"),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Transparent,
                emission_level,
            },
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn candle_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             WATERLOGGED_PROP,
        ]),
        replacement_variants: Some(vec![
            CustomProperty::int("candles", 1..=4),
            LIT_PROP,
        ]),
        default_override: Some(vec![
            ("candles", "1").into(),
            ("lit", "false").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        extra_info_modifiers: vec![
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    light_info: Some(BlockLightInfo {
                        sky_light_opacity: SkyLightOpacity::Transparent,
                        emission_level: 12,
                    }),
                    ..Default::default()
                },
                conditions: vec![
                    ("candles", "4").into(),
                    ("lit", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    light_info: Some(BlockLightInfo {
                        sky_light_opacity: SkyLightOpacity::Transparent,
                        emission_level: 9,
                    }),
                    ..Default::default()
                },
                conditions: vec![
                    ("candles", "3").into(),
                    ("lit", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    light_info: Some(BlockLightInfo {
                        sky_light_opacity: SkyLightOpacity::Transparent,
                        emission_level: 6,
                    }),
                    ..Default::default()
                },
                conditions: vec![
                    ("candles", "2").into(),
                    ("lit", "true").into(),
                ],
            },
            BlockstateInfoModifierCase {
                modifier: BlockstateInfoModifier {
                    light_info: Some(BlockLightInfo {
                        sky_light_opacity: SkyLightOpacity::Transparent,
                        emission_level: 3,
                    }),
                    ..Default::default()
                },
                conditions: vec![
                    ("candles", "1").into(),
                    ("lit", "true").into(),
                ],
            },
        ],
        ..StandardRegistration::new(identifier)
    })
}

fn candle_cake_reg(identifier: &str) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        replacement_variants: Some(vec![
            LIT_PROP,
        ]),
        default_override: Some(vec![
            ("lit", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: SKY_TRANSPARENT_INFO,
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn amethyst_bud_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        custom_variants: Some(vec![
                             WATERLOGGED_PROP,
        ]),
        replacement_variants: Some(vec![
                                  facing_neswud_prop(),
        ]),
        default_override: Some(vec![
            ("facing", "up").into(),
            ("waterlogged", "false").into(),
        ]),
        default_extra_info: BlockstateInfo {
            opacity: BlockOpacity::Transparent,
            light_info: BlockLightInfo {
                sky_light_opacity: SkyLightOpacity::Transparent,
                emission_level,
            },
            ..Default::default()
        },
        ..StandardRegistration::new(identifier)
    })
}

fn bulb_reg(identifier: &str, emission_level: u8) -> Registration<'_> {
    Registration::Standard(StandardRegistration {
        replacement_variants: Some(vec![
            LIT_PROP,
            POWERED_PROP,
        ]),
        default_override: Some(vec![
            ("lit", "false").into(),
            ("powered", "false").into(),
        ]),
        extra_info_modifiers: vec![BlockstateInfoModifierCase {
            modifier: BlockstateInfoModifier {
                light_info: Some(BlockLightInfo {
                    sky_light_opacity: SkyLightOpacity::Opaque,
                    emission_level,
                }),
                ..Default::default()
            },
            conditions: vec![("lit", "true").into()],
        }],
        ..StandardRegistration::new(identifier)
    })
}

pub(super) fn registrations() -> Vec<Registration<'static>> {
    vec![
        Registration::Standard(StandardRegistration {
            properties: Properties {
                air_like: true,
            },
            default_extra_info: BlockstateInfo {
                opacity: BlockOpacity::Transparent,
                light_info: BlockLightInfo {
                    sky_light_opacity: SkyLightOpacity::Transparent,
                    emission_level: 0,
                },
                collision_info: CollisionInfo::Empty,
            },
        }),
  basic_reg("stone"),
  basic_reg("granite"),
  basic_reg("polished_granite"),
  basic_reg("diorite"),
  basic_reg("polished_diorite"),
  basic_reg("andesite"),
  basic_reg("polished_andesite"),
  Registration::Standard(StandardRegistration {  default_override: Some(vec![ ("snowy", "false").into() ]), replacement_variants: Some(vec![CustomProperty::boolean("snowy")]), ..StandardRegistration::new("grass_block") }),
  basic_reg("dirt"),
  basic_reg("coarse_dirt"),
  Registration::Standard(StandardRegistration {  default_override: Some(vec![ ("snowy", "false").into() ]), replacement_variants: Some(vec![CustomProperty::boolean("snowy")]), ..StandardRegistration::new("podzol") }),
  basic_reg("cobblestone"),
  basic_reg("oak_planks"),
  basic_reg("spruce_planks"),
  basic_reg("birch_planks"),
  basic_reg("jungle_planks"),
  basic_reg("acacia_planks"),
  basic_reg("cherry_planks"),
  basic_reg("dark_oak_planks"),
  basic_reg("mangrove_planks"),
  basic_reg("bamboo_planks"),
  basic_reg("bamboo_mosaic"),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("oak_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("spruce_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("birch_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("jungle_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("acacia_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("cherry_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("dark_oak_sapling") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![STAGE_0_1_PROP, WATERLOGGED_PROP]), replacement_variants: Some(vec![CustomProperty::int("age", 0..=4), CustomProperty::boolean("hanging")]), default_override: Some(vec![ ("age", "0").into(), ("hanging", "false").into(), ("stage", "0").into(), ("waterlogged", "false").into(), ]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, collision_info: CollisionInfo::Empty, }, ..StandardRegistration::new("mangrove_propagule") }),
  basic_reg("bedrock"),
  basic_liquid_reg("water"),
  basic_liquid_light_reg("lava", 15),
  basic_reg("sand"),
  basic_reg("suspicious_sand"),
  basic_reg("red_sand"),
  basic_reg("gravel"),
  basic_reg("suspicious_gravel"),
  basic_reg("gold_ore"),
  basic_reg("deepslate_gold_ore"),
  basic_reg("iron_ore"),
  basic_reg("deepslate_iron_ore"),
  basic_reg("coal_ore"),
  basic_reg("deepslate_coal_ore"),
  basic_reg("nether_gold_ore"),
  log_reg("oak_log"),
  log_reg("spruce_log"),
  log_reg("birch_log"),
  log_reg("jungle_log"),
  log_reg("acacia_log"),
  log_reg("cherry_log"),
  log_reg("dark_oak_log"),
  log_reg("mangrove_log"),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![WATERLOGGED_PROP]), default_override: Some(vec![ ("waterlogged", "false").into() ]), ..StandardRegistration::new("mangrove_roots") }),
  log_reg("muddy_mangrove_roots"),
  log_reg("bamboo_block"),
  log_reg("stripped_spruce_log"),
  log_reg("stripped_birch_log"),
  log_reg("stripped_jungle_log"),
  log_reg("stripped_acacia_log"),
  log_reg("stripped_cherry_log"),
  log_reg("stripped_dark_oak_log"),
  log_reg("stripped_oak_log"),
  log_reg("stripped_mangrove_log"),
  log_reg("stripped_bamboo_block"),
  log_reg("oak_wood"),
  log_reg("spruce_wood"),
  log_reg("birch_wood"),
  log_reg("jungle_wood"),
  log_reg("acacia_wood"),
  log_reg("cherry_wood"),
  log_reg("dark_oak_wood"),
  log_reg("mangrove_wood"),
  log_reg("stripped_oak_wood"),
  log_reg("stripped_spruce_wood"),
  log_reg("stripped_birch_wood"),
  log_reg("stripped_jungle_wood"),
  log_reg("stripped_acacia_wood"),
  log_reg("stripped_cherry_wood"),
  log_reg("stripped_dark_oak_wood"),
  log_reg("stripped_mangrove_wood"),
  leaves_reg("oak_leaves"),
  leaves_reg("spruce_leaves"),
  leaves_reg("birch_leaves"),
  leaves_reg("jungle_leaves"),
  leaves_reg("acacia_leaves"),
  leaves_reg("cherry_leaves"),
  leaves_reg("dark_oak_leaves"),
  leaves_reg("mangrove_leaves"),
  leaves_reg("azalea_leaves"),
  leaves_reg("flowering_azalea_leaves"),
  basic_reg("sponge"),
  basic_reg("wet_sponge"),
  Registration::Standard(StandardRegistration {  default_extra_info: BlockstateInfo { opacity: BlockOpacity::Glass }, ..StandardRegistration::new("glass") }),
  basic_reg("lapis_ore"),
  basic_reg("deepslate_lapis_ore"),
  basic_reg("lapis_block"),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![CustomProperty::boolean("triggered")]), replacement_variants: Some(vec![facing_neswud_prop()]), default_override: Some(vec![ ("facing", "north").into(), ("triggered", "false").into() ]), ..StandardRegistration::new("dispenser") }),
  basic_reg("sandstone"),
  basic_reg("chiseled_sandstone"),
  basic_reg("cut_sandstone"),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![ CustomProperty::enum_variants("instrument", vec![ "harp", "basedrum", "snare", "hat", "bass", "flute", "bell", "guitar", "chime", "xylophone", "iron_xylophone", "cow_bell", "didgeridoo", "bit", "banjo", "pling", "zombie", "skeleton", "creeper", "dragon", "wither_skeleton", "piglin", "custom_head", ]), CustomProperty::int("note", 0..=24), POWERED_PROP, ]), default_override: Some(vec![ ("instrument", "harp").into(), ("note", "0").into(), ("powered", "false").into() ]), ..StandardRegistration::new("note_block") }),
  bed_reg("white_bed"),
  bed_reg("orange_bed"),
  bed_reg("magenta_bed"),
  bed_reg("light_blue_bed"),
  bed_reg("yellow_bed"),
  bed_reg("lime_bed"),
  bed_reg("pink_bed"),
  bed_reg("gray_bed"),
  bed_reg("light_gray_bed"),
  bed_reg("cyan_bed"),
  bed_reg("purple_bed"),
  bed_reg("blue_bed"),
  bed_reg("brown_bed"),
  bed_reg("green_bed"),
  bed_reg("red_bed"),
  bed_reg("black_bed"),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![WATERLOGGED_PROP]), replacement_variants: Some(vec![ POWERED_PROP, CustomProperty::enum_variants("shape", vec![ "north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south", ]), ]), default_override: Some(vec![ ("powered", "false").into(), ("shape", "north_south").into(), ("waterlogged", "false").into(), ]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, }, ..StandardRegistration::new("powered_rail") }),
  Registration::Standard(StandardRegistration {  custom_variants: Some(vec![WATERLOGGED_PROP]), replacement_variants: Some(vec![ POWERED_PROP, CustomProperty::enum_variants("shape", vec![ "north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south", ]), ]), default_override: Some(vec![ ("powered", "false").into(), ("shape", "north_south").into(), ("waterlogged", "false").into(), ]), default_extra_info: BlockstateInfo { opacity: BlockOpacity::Transparent, light_info: SKY_TRANSPARENT_INFO, }, ..StandardRegistration::new("detector_rail") }),
  {
    identifier = "sticky_piston",
    replacement_variants = [bool_prop "extended", facing_neswud],
    default_override =
      make_kv_list { extended = "false", facing = "north" },
    extra_info_modifiers = [
      {
        modifier = {
          opacity = 'Transparent,
          light_info = sky_transparent_info,
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.25] [1.0, 1.0, 1.0],
            ],
        },
        conditions = make_kv_list { extended = "true" },
      }
    ],
  } | StandardRegistration,
  transparent_no_collider_reg("cobweb"),
  transparent_no_collider_reg("short_grass"),
  transparent_no_collider_reg("fern"),
  transparent_no_collider_reg("dead_bush"),
  transparent_no_collider_reg("seagrass"),
  {
    identifier = "tall_seagrass",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "piston",
    replacement_variants = [bool_prop "extended", facing_neswud],
    default_override =
      make_kv_list { extended = "false", facing = "north" },
    extra_info_modifiers = [
      {
        modifier = {
          opacity = 'Transparent,
          light_info = sky_transparent_info,
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.25] [1.0, 1.0, 1.0],
            ],
        },
        conditions = make_kv_list { extended = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "piston_head",
    replacement_variants = [
      facing_neswud,
      bool_prop "short",
      enum_prop "type" ["normal", "sticky"],
    ],
    default_override =
      make_kv_list { facing = "north", short = "false", type = "normal" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info =
        complex_collision_info [
          /* Stem */
          make_aabb [0.375, 0.375, 0.25] [0.625, 0.625, 1.0],
          /* Head */
          make_aabb [0.0, 0.0, 0.0] [1.0, 1.0, 0.25],
        ],
    },
  } | StandardRegistration,
  basic_reg("white_wool"),
  basic_reg("orange_wool"),
  basic_reg("magenta_wool"),
  basic_reg("light_blue_wool"),
  basic_reg("yellow_wool"),
  basic_reg("lime_wool"),
  basic_reg("pink_wool"),
  basic_reg("gray_wool"),
  basic_reg("light_gray_wool"),
  basic_reg("cyan_wool"),
  basic_reg("purple_wool"),
  basic_reg("blue_wool"),
  basic_reg("brown_wool"),
  basic_reg("green_wool"),
  basic_reg("red_wool"),
  basic_reg("black_wool"),
  {
    identifier = "moving_piston",
    custom_variants = [facing_neswud, enum_prop "type" ["normal", "sticky"]],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_no_collider_reg("dandelion"),
  transparent_no_collider_reg("torchflower"),
  transparent_no_collider_reg("poppy"),
  transparent_no_collider_reg("blue_orchid"),
  transparent_no_collider_reg("allium"),
  transparent_no_collider_reg("azure_bluet"),
  transparent_no_collider_reg("red_tulip"),
  transparent_no_collider_reg("orange_tulip"),
  transparent_no_collider_reg("white_tulip"),
  transparent_no_collider_reg("pink_tulip"),
  transparent_no_collider_reg("oxeye_daisy"),
  transparent_no_collider_reg("cornflower"),
  transparent_no_collider_reg("wither_rose"),
  transparent_no_collider_reg("lily_of_the_valley"),
  transparent_light_reg("brown_mushroom", 1),
  transparent_no_collider_reg("red_mushroom"),
  basic_reg("gold_block"),
  basic_reg("iron_block"),
  basic_reg("bricks"),
  {
    identifier = "tnt",
    custom_variants = [bool_prop "unstable"],
    default_override = make_kv_list { unstable = "false" },
  } | StandardRegistration,
  basic_reg("bookshelf"),
  {
    identifier = "chiseled_bookshelf",
    custom_variants = [
      facing_nswe,
      bool_prop "slot_0_occupied",
      bool_prop "slot_1_occupied",
      bool_prop "slot_2_occupied",
      bool_prop "slot_3_occupied",
      bool_prop "slot_4_occupied",
      bool_prop "slot_5_occupied",
    ],
    default_override =
      make_kv_list {
        facing = "north",
        slot_0_occupied = "false",
        slot_1_occupied = "false",
        slot_2_occupied = "false",
        slot_3_occupied = "false",
        slot_4_occupied = "false",
        slot_5_occupied = "false",
      },
  } | StandardRegistration,
  basic_reg("mossy_cobblestone"),
  basic_reg("obsidian"),
  transparent_light_no_collider_reg("torch", 14),
  {
    identifier = "wall_torch",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 14,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "fire",
    custom_variants = [
      age_0_15,
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        age = "0",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "soul_fire",
    custom_variants = [],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 10,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  transparent_reg("spawner"),
  stairs_reg("oak_stairs"),
  chest_reg("chest"),
  {
    identifier = "redstone_wire",
    custom_variants = [
      enum_prop "east" ["up", "side", "none"],
      enum_prop "north" ["up", "side", "none"],
      int_prop "power" 0 15,
      enum_prop "south" ["up", "side", "none"],
      enum_prop "west" ["up", "side", "none"],
    ],
    default_override =
      make_kv_list {
        east = "none",
        north = "none",
        south = "none",
        west = "none",
        power = "0",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("diamond_ore"),
  basic_reg("deepslate_diamond_ore"),
  basic_reg("diamond_block"),
  basic_reg("crafting_table"),
  transparent_no_collider_reg("wheat"),
  transparent_reg("farmland"),
  {
    identifier = "furnace",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  sign_reg("oak_sign"),
  sign_reg("spruce_sign"),
  sign_reg("birch_sign"),
  sign_reg("acacia_sign"),
  sign_reg("cherry_sign"),
  sign_reg("jungle_sign"),
  sign_reg("dark_oak_sign"),
  sign_reg("mangrove_sign"),
  sign_reg("bamboo_sign"),
  door_reg("oak_door"),
  {
    identifier = "ladder",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
          "south_east",
          "south_west",
          "north_west",
          "north_east",
        ],
    ],
    default_override =
      make_kv_list { shape = "north_south", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  stairs_reg("cobblestone_stairs"),
  wall_sign_reg("oak_wall_sign"),
  wall_sign_reg("spruce_wall_sign"),
  wall_sign_reg("birch_wall_sign"),
  wall_sign_reg("acacia_wall_sign"),
  wall_sign_reg("cherry_wall_sign"),
  wall_sign_reg("jungle_wall_sign"),
  wall_sign_reg("dark_oak_wall_sign"),
  wall_sign_reg("mangrove_wall_sign"),
  wall_sign_reg("bamboo_wall_sign"),
  hanging_sign_reg("oak_hanging_sign"),
  hanging_sign_reg("spruce_hanging_sign"),
  hanging_sign_reg("birch_hanging_sign"),
  hanging_sign_reg("acacia_hanging_sign"),
  hanging_sign_reg("cherry_hanging_sign"),
  hanging_sign_reg("jungle_hanging_sign"),
  hanging_sign_reg("dark_oak_hanging_sign"),
  hanging_sign_reg("crimson_hanging_sign"),
  hanging_sign_reg("warped_hanging_sign"),
  hanging_sign_reg("mangrove_hanging_sign"),
  hanging_sign_reg("bamboo_hanging_sign"),
  wall_hanging_sign_reg("oak_wall_hanging_sign"),
  wall_hanging_sign_reg("spruce_wall_hanging_sign"),
  wall_hanging_sign_reg("birch_wall_hanging_sign"),
  wall_hanging_sign_reg("acacia_wall_hanging_sign"),
  wall_hanging_sign_reg("cherry_wall_hanging_sign"),
  wall_hanging_sign_reg("jungle_wall_hanging_sign"),
  wall_hanging_sign_reg("dark_oak_wall_hanging_sign"),
  wall_hanging_sign_reg("mangrove_wall_hanging_sign"),
  wall_hanging_sign_reg("crimson_wall_hanging_sign"),
  wall_hanging_sign_reg("warped_wall_hanging_sign"),
  wall_hanging_sign_reg("bamboo_wall_hanging_sign"),
  {
    identifier = "lever",
    replacement_variants = [
      enum_prop "face" ["floor", "wall", "ceiling"],
      facing_nswe,
      powered,
    ],
    default_override =
      make_kv_list { face = "wall", facing = "north", powered = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  pressure_plate_reg("stone_pressure_plate"),
  {
    identifier = "iron_door",
    custom_variants = [powered],
    replacement_variants = [
      facing_nswe,
      enum_prop "half" ["upper", "lower"],
      enum_prop "hinge" ["left", "right"],
      bool_prop "open",
    ],
    default_override =
      make_kv_list {
        facing = "north",
        half = "lower",
        hinge = "left",
        open = "false",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  pressure_plate_reg("oak_pressure_plate"),
  pressure_plate_reg("spruce_pressure_plate"),
  pressure_plate_reg("birch_pressure_plate"),
  pressure_plate_reg("jungle_pressure_plate"),
  pressure_plate_reg("acacia_pressure_plate"),
  pressure_plate_reg("cherry_pressure_plate"),
  pressure_plate_reg("dark_oak_pressure_plate"),
  pressure_plate_reg("mangrove_pressure_plate"),
  pressure_plate_reg("bamboo_pressure_plate"),
  {
    identifier = "redstone_ore",
    custom_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "deepslate_redstone_ore",
    custom_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "redstone_torch",
    replacement_variants = [lit],
    default_extra_info = {
      opacity = 'Transparent,
      collision_info = empty_collision_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 7
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "redstone_wall_torch",
    replacement_variants = [facing_nswe, lit],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 7
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  button_reg("stone_button"),
  {
    identifier = "snow",
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          collision_info = empty_collision_info,
        },
        conditions = make_kv_list { layers = "1" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.125, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "2" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.25, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "3" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.375, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "4" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "5" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.625, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "6" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.75, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "7" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.875, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "8" },
      },
    ],
  } | StandardRegistration,
  transparent_reg("ice"),
  basic_reg("snow_block"),
  {
    identifier = "cactus",
    custom_variants = [age_0_15],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("clay"),
  {
    identifier = "sugar_cane",
    custom_variants = [age_0_15],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "jukebox",
    custom_variants = [bool_prop "has_record"],
    default_override = make_kv_list { has_record = "false" },
  } | StandardRegistration,
  fence_reg("oak_fence"),
  basic_reg("netherrack"),
  transparent_reg("soul_sand"),
  basic_reg("soul_soil"),
  {
    identifier = "basalt",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "polished_basalt",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  transparent_light_no_collider_reg("soul_torch", 10),
  {
    identifier = "soul_wall_torch",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 10,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  basic_light_reg("glowstone", 15),
  transparent_light_reg("nether_portal", 11),
  {
    identifier = "carved_pumpkin",
    replacement_variants = [facing_nswe],
  } | StandardRegistration,
  {
    identifier = "jack_o_lantern",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  transparent_reg("cake"),
  {
    identifier = "repeater",
    replacement_variants = [
      int_prop "delay" 1 4,
      facing_nswe,
      bool_prop "locked",
      bool_prop "powered",
    ],
    default_override =
      make_kv_list {
        delay = "1",
        facing = "north",
        locked = "false",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  stained_glass_reg("white_stained_glass"),
  stained_glass_reg("orange_stained_glass"),
  stained_glass_reg("magenta_stained_glass"),
  stained_glass_reg("light_blue_stained_glass"),
  stained_glass_reg("yellow_stained_glass"),
  stained_glass_reg("lime_stained_glass"),
  stained_glass_reg("pink_stained_glass"),
  stained_glass_reg("gray_stained_glass"),
  stained_glass_reg("light_gray_stained_glass"),
  stained_glass_reg("cyan_stained_glass"),
  stained_glass_reg("purple_stained_glass"),
  stained_glass_reg("blue_stained_glass"),
  stained_glass_reg("brown_stained_glass"),
  stained_glass_reg("green_stained_glass"),
  stained_glass_reg("red_stained_glass"),
  stained_glass_reg("black_stained_glass"),
  trapdoor_reg("oak_trapdoor"),
  trapdoor_reg("spruce_trapdoor"),
  trapdoor_reg("birch_trapdoor"),
  trapdoor_reg("jungle_trapdoor"),
  trapdoor_reg("acacia_trapdoor"),
  trapdoor_reg("cherry_trapdoor"),
  trapdoor_reg("dark_oak_trapdoor"),
  trapdoor_reg("mangrove_trapdoor"),
  trapdoor_reg("bamboo_trapdoor"),
  basic_reg("stone_bricks"),
  basic_reg("mossy_stone_bricks"),
  basic_reg("cracked_stone_bricks"),
  basic_reg("chiseled_stone_bricks"),
  basic_reg("packed_mud"),
  basic_reg("mud_bricks"),
  basic_reg("infested_stone"),
  basic_reg("infested_cobblestone"),
  basic_reg("infested_stone_bricks"),
  basic_reg("infested_mossy_stone_bricks"),
  basic_reg("infested_cracked_stone_bricks"),
  basic_reg("infested_chiseled_stone_bricks"),
  mushroom_block_reg("brown_mushroom_block"),
  mushroom_block_reg("red_mushroom_block"),
  mushroom_block_reg("mushroom_stem"),
  {
    identifier = "iron_bars",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "chain",
    custom_variants = [waterlogged],
    default_override =
      make_kv_list { axis = "y", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "glass_pane",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'GlassPane,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("pumpkin"),
  basic_reg("melon"),
  {
    identifier = "attached_pumpkin_stem",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "attached_melon_stem",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("pumpkin_stem"),
  transparent_reg("melon_stem"),
  {
    identifier = "vine",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "glow_lichen",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        up = "false",
        down = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 7
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  fence_gate_reg("oak_fence_gate"),
  stairs_reg("brick_stairs"),
  stairs_reg("stone_brick_stairs"),
  stairs_reg("mud_brick_stairs"),
  {
    identifier = "mycelium",
    replacement_variants = [bool_prop "snowy"],
    default_override = make_kv_list { snowy = "false" },
  } | StandardRegistration,
  transparent_reg("lily_pad"),
  basic_reg("nether_bricks"),
  fence_reg("nether_brick_fence"),
  stairs_reg("nether_brick_stairs"),
  transparent_reg("nether_wart"),
  transparent_light_reg("enchanting_table", 7),
  {
    identifier = "brewing_stand",
    custom_variants = [
      bool_prop "has_bottle_0",
      bool_prop "has_bottle_1",
      bool_prop "has_bottle_2",
    ],
    default_override =
      make_kv_list {
        has_bottle_0 = "false",
        has_bottle_1 = "false",
        has_bottle_2 = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | StandardRegistration,
  transparent_reg("cauldron"),
  transparent_reg("water_cauldron"),
  transparent_light_reg("lava_cauldron", 15),
  transparent_reg("powder_snow_cauldron"),
  basic_light_reg("end_portal", 15),
  {
    identifier = "end_portal_frame",
    replacement_variants = [bool_prop "eye", facing_nswe],
    default_override =
      make_kv_list { eye = "false", facing = "north" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | StandardRegistration,
  basic_reg("end_stone"),
  transparent_light_reg("dragon_egg", 1),
  {
    identifier = "redstone_lamp",
    replacement_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "cocoa",
    replacement_variants = [int_prop "age" 0 2, facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  stairs_reg("sandstone_stairs"),
  basic_reg("emerald_ore"),
  basic_reg("deepslate_emerald_ore"),
  {
    identifier = "ender_chest",
    custom_variants = [facing_nswe, waterlogged],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 7
      },
    },
  } | StandardRegistration,
  {
    identifier = "tripwire_hook",
    replacement_variants = [bool_prop "attached", facing_nswe, powered],
    default_override =
      make_kv_list { attached = "false", facing = "north", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  },
  {
    identifier = "tripwire",
    custom_variants = [
      bool_prop "attached",
      bool_prop "disarmed",
      bool_prop "east",
      bool_prop "north",
      powered,
      bool_prop "south",
      bool_prop "west",
    ],
    skip_properties = ["disarmed", "powered"],
    default_override =
      make_kv_list {
        attached = "false",
        disarmed = "false",
        east = "false",
        north = "false",
        powered = "false",
        south = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  basic_reg("emerald_block"),
  stairs_reg("spruce_stairs"),
  stairs_reg("birch_stairs"),
  stairs_reg("jungle_stairs"),
  {
    identifier = "command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  transparent_light_reg("beacon", 15),
  wall_reg("cobblestone_wall"),
  wall_reg("mossy_cobblestone_wall"),
  transparent_reg("flower_pot"),
  transparent_reg("potted_torchflower"),
  transparent_reg("potted_oak_sapling"),
  transparent_reg("potted_spruce_sapling"),
  transparent_reg("potted_birch_sapling"),
  transparent_reg("potted_jungle_sapling"),
  transparent_reg("potted_acacia_sapling"),
  transparent_reg("potted_cherry_sapling"),
  transparent_reg("potted_dark_oak_sapling"),
  transparent_reg("potted_mangrove_propagule"),
  transparent_reg("potted_fern"),
  transparent_reg("potted_dandelion"),
  transparent_reg("potted_poppy"),
  transparent_reg("potted_blue_orchid"),
  transparent_reg("potted_allium"),
  transparent_reg("potted_azure_bluet"),
  transparent_reg("potted_red_tulip"),
  transparent_reg("potted_orange_tulip"),
  transparent_reg("potted_white_tulip"),
  transparent_reg("potted_pink_tulip"),
  transparent_reg("potted_oxeye_daisy"),
  transparent_reg("potted_cornflower"),
  transparent_reg("potted_lily_of_the_valley"),
  transparent_reg("potted_wither_rose"),
  transparent_reg("potted_red_mushroom"),
  transparent_reg("potted_brown_mushroom"),
  transparent_reg("potted_dead_bush"),
  transparent_reg("potted_cactus"),
  transparent_reg("carrots"),
  transparent_reg("potatoes"),
  button_reg("oak_button"),
  button_reg("spruce_button"),
  button_reg("birch_button"),
  button_reg("jungle_button"),
  button_reg("acacia_button"),
  button_reg("cherry_button"),
  button_reg("dark_oak_button"),
  button_reg("mangrove_button"),
  button_reg("bamboo_button"),
  head_reg("skeleton_skull"),
  wall_head_reg("skeleton_wall_skull"),
  head_reg("wither_skeleton_skull"),
  wall_head_reg("wither_skeleton_wall_skull"),
  head_reg("zombie_head"),
  wall_head_reg("zombie_wall_head"),
  head_reg("player_head"),
  wall_head_reg("player_wall_head"),
  head_reg("creeper_head"),
  wall_head_reg("creeper_wall_head"),
  head_reg("dragon_head"),
  wall_head_reg("dragon_wall_head"),
  head_reg("piglin_head"),
  wall_head_reg("piglin_wall_head"),
  {
    identifier = "anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "chipped_anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "damaged_anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  chest_reg("trapped_chest"),
  weighted_pressure_plate_reg("light_weighted_pressure_plate"),
  weighted_pressure_plate_reg("heavy_weighted_pressure_plate"),
  {
    identifier = "comparator",
    replacement_variants = [facing_nswe, enum_prop "mode" ["compare", "subtract"], powered],
    default_override =
      make_kv_list { facing = "north", mode = "compare", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "daylight_detector",
    custom_variants = [int_prop "power" 0 15],
    replacement_variants = [bool_prop "inverted"],
    default_override =
      make_kv_list { inverted = "false", power = "0" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("redstone_block"),
  basic_reg("nether_quartz_ore"),
  {
    identifier = "hopper",
    custom_variants = [
      bool_prop "enabled",
      enum_prop "facing" ["down", "north", "south", "west", "east"],
    ],
    skip_properties = ["enabled"],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  basic_reg("quartz_block"),
  basic_reg("chiseled_quartz_block"),
  {
    identifier = "quartz_pillar",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "quartz_stairs",
    custom_variants = [waterlogged],
    replacement_variants = [
      facing_nswe,
      enum_prop "half" ["top", "bottom"],
      enum_prop
        "shape"
        [
          "straight",
          "inner_left",
          "inner_right",
          "outer_left",
          "outer_right",
        ],
    ],
    default_override =
      make_kv_list {
        facing = "north",
        half = "bottom",
        shape = "straight",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "activator_rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      powered,
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
        ],
    ],
    default_override =
      make_kv_list {
        powered = "false",
        shape = "north_south",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "dropper",
    custom_variants = [bool_prop "triggered"],
    replacement_variants = [facing_neswud],
    default_override =
      make_kv_list { facing = "north", triggered = "false" },
  } | StandardRegistration,
  basic_reg("white_terracotta"),
  basic_reg("orange_terracotta"),
  basic_reg("magenta_terracotta"),
  basic_reg("light_blue_terracotta"),
  basic_reg("yellow_terracotta"),
  basic_reg("lime_terracotta"),
  basic_reg("pink_terracotta"),
  basic_reg("gray_terracotta"),
  basic_reg("light_gray_terracotta"),
  basic_reg("cyan_terracotta"),
  basic_reg("purple_terracotta"),
  basic_reg("blue_terracotta"),
  basic_reg("brown_terracotta"),
  basic_reg("green_terracotta"),
  basic_reg("red_terracotta"),
  basic_reg("black_terracotta"),
  glass_pane_reg("white_stained_glass_pane"),
  glass_pane_reg("orange_stained_glass_pane"),
  glass_pane_reg("magenta_stained_glass_pane"),
  glass_pane_reg("light_blue_stained_glass_pane"),
  glass_pane_reg("yellow_stained_glass_pane"),
  glass_pane_reg("lime_stained_glass_pane"),
  glass_pane_reg("pink_stained_glass_pane"),
  glass_pane_reg("gray_stained_glass_pane"),
  glass_pane_reg("light_gray_stained_glass_pane"),
  glass_pane_reg("cyan_stained_glass_pane"),
  glass_pane_reg("purple_stained_glass_pane"),
  glass_pane_reg("blue_stained_glass_pane"),
  glass_pane_reg("brown_stained_glass_pane"),
  glass_pane_reg("green_stained_glass_pane"),
  glass_pane_reg("red_stained_glass_pane"),
  glass_pane_reg("black_stained_glass_pane"),
  stairs_reg("acacia_stairs"),
  stairs_reg("cherry_stairs"),
  stairs_reg("dark_oak_stairs"),
  stairs_reg("mangrove_stairs"),
  stairs_reg("bamboo_stairs"),
  stairs_reg("bamboo_mosaic_stairs"),
  transparent_reg("slime_block"),
  {
    identifier = "barrier",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "light",
    custom_variants = [waterlogged],
    replacement_variants = [int_prop "level" 0 15],
    default_override =
      make_kv_list { level = "15", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = sky_transparent_info,
        },
        conditions = make_kv_list { level = "0" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 1
          },
        },
        conditions = make_kv_list { level = "1" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 2
          },
        },
        conditions = make_kv_list { level = "2" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 3
          },
        },
        conditions = make_kv_list { level = "3" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 4
          },
        },
        conditions = make_kv_list { level = "4" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 5
          },
        },
        conditions = make_kv_list { level = "5" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 6
          },
        },
        conditions = make_kv_list { level = "6" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 7
          },
        },
        conditions = make_kv_list { level = "7" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 8
          },
        },
        conditions = make_kv_list { level = "8" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 9
          },
        },
        conditions = make_kv_list { level = "9" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 10,
          },
        },
        conditions = make_kv_list { level = "10" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 11,
          },
        },
        conditions = make_kv_list { level = "11" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 12,
          },
        },
        conditions = make_kv_list { level = "12" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { level = "13" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 14,
          },
        },
        conditions = make_kv_list { level = "14" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { level = "15" },
      },
    ],
  } | StandardRegistration,
  trapdoor_reg("iron_trapdoor"),
  basic_reg("prismarine"),
  basic_reg("prismarine_bricks"),
  basic_reg("dark_prismarine"),
  stairs_reg("prismarine_stairs"),
  stairs_reg("prismarine_brick_stairs"),
  stairs_reg("dark_prismarine_stairs"),
  slab_reg("prismarine_slab"),
  slab_reg("prismarine_brick_slab"),
  slab_reg("dark_prismarine_slab"),
  transparent_light_reg("sea_lantern", 15),
  {
    identifier = "hay_block",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  transparent_reg("white_carpet"),
  transparent_reg("orange_carpet"),
  transparent_reg("magenta_carpet"),
  transparent_reg("light_blue_carpet"),
  transparent_reg("yellow_carpet"),
  transparent_reg("lime_carpet"),
  transparent_reg("pink_carpet"),
  transparent_reg("gray_carpet"),
  transparent_reg("light_gray_carpet"),
  transparent_reg("cyan_carpet"),
  transparent_reg("purple_carpet"),
  transparent_reg("blue_carpet"),
  transparent_reg("brown_carpet"),
  transparent_reg("green_carpet"),
  transparent_reg("red_carpet"),
  transparent_reg("black_carpet"),
  basic_reg("terracotta"),
  basic_reg("coal_block"),
  basic_reg("packed_ice"),
  {
    identifier = "sunflower",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "lilac",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "rose_bush",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "peony",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "tall_grass",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "large_fern",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  banner_reg("white_banner"),
  banner_reg("orange_banner"),
  banner_reg("magenta_banner"),
  banner_reg("light_blue_banner"),
  banner_reg("yellow_banner"),
  banner_reg("lime_banner"),
  banner_reg("pink_banner"),
  banner_reg("gray_banner"),
  banner_reg("light_gray_banner"),
  banner_reg("cyan_banner"),
  banner_reg("purple_banner"),
  banner_reg("blue_banner"),
  banner_reg("brown_banner"),
  banner_reg("green_banner"),
  banner_reg("red_banner"),
  banner_reg("black_banner"),
  wall_banner_reg("white_wall_banner"),
  wall_banner_reg("orange_wall_banner"),
  wall_banner_reg("magenta_wall_banner"),
  wall_banner_reg("light_blue_wall_banner"),
  wall_banner_reg("yellow_wall_banner"),
  wall_banner_reg("lime_wall_banner"),
  wall_banner_reg("pink_wall_banner"),
  wall_banner_reg("gray_wall_banner"),
  wall_banner_reg("light_gray_wall_banner"),
  wall_banner_reg("cyan_wall_banner"),
  wall_banner_reg("purple_wall_banner"),
  wall_banner_reg("blue_wall_banner"),
  wall_banner_reg("brown_wall_banner"),
  wall_banner_reg("green_wall_banner"),
  wall_banner_reg("red_wall_banner"),
  wall_banner_reg("black_wall_banner"),
  basic_reg("red_sandstone"),
  basic_reg("chiseled_red_sandstone"),
  basic_reg("cut_red_sandstone"),
  stairs_reg("red_sandstone_stairs"),
  slab_reg("oak_slab"),
  slab_reg("spruce_slab"),
  slab_reg("birch_slab"),
  slab_reg("jungle_slab"),
  slab_reg("acacia_slab"),
  slab_reg("cherry_slab"),
  slab_reg("dark_oak_slab"),
  slab_reg("mangrove_slab"),
  slab_reg("bamboo_slab"),
  slab_reg("bamboo_mosaic_slab"),
  slab_reg("stone_slab"),
  slab_reg("smooth_stone_slab"),
  slab_reg("sandstone_slab"),
  slab_reg("cut_sandstone_slab"),
  slab_reg("petrified_oak_slab"),
  slab_reg("cobblestone_slab"),
  slab_reg("brick_slab"),
  slab_reg("stone_brick_slab"),
  slab_reg("mud_brick_slab"),
  slab_reg("nether_brick_slab"),
  slab_reg("quartz_slab"),
  slab_reg("red_sandstone_slab"),
  slab_reg("cut_red_sandstone_slab"),
  slab_reg("purpur_slab"),
  basic_reg("smooth_stone"),
  basic_reg("smooth_sandstone"),
  basic_reg("smooth_quartz"),
  basic_reg("smooth_red_sandstone"),
  fence_gate_reg("spruce_fence_gate"),
  fence_gate_reg("birch_fence_gate"),
  fence_gate_reg("jungle_fence_gate"),
  fence_gate_reg("acacia_fence_gate"),
  fence_gate_reg("cherry_fence_gate"),
  fence_gate_reg("dark_oak_fence_gate"),
  fence_gate_reg("mangrove_fence_gate"),
  fence_gate_reg("bamboo_fence_gate"),
  fence_reg("spruce_fence"),
  fence_reg("birch_fence"),
  fence_reg("jungle_fence"),
  fence_reg("acacia_fence"),
  fence_reg("cherry_fence"),
  fence_reg("dark_oak_fence"),
  fence_reg("mangrove_fence"),
  fence_reg("bamboo_fence"),
  door_reg("spruce_door"),
  door_reg("birch_door"),
  door_reg("jungle_door"),
  door_reg("acacia_door"),
  door_reg("cherry_door"),
  door_reg("dark_oak_door"),
  door_reg("mangrove_door"),
  door_reg("bamboo_door"),
  {
    identifier = "end_rod",
    replacement_variants = [facing_neswud],
    default_override = make_kv_list { facing = "up" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 14,
      },
    },
  } | StandardRegistration,
  {
    identifier = "chorus_plant",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        down = "false",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  transparent_reg("chorus_flower"),
  basic_reg("purpur_block"),
  {
    identifier = "purpur_pillar",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  stairs_reg("purpur_stairs"),
  basic_reg("end_stone_bricks"),
  transparent_reg("torchflower_crop"),
  {
    identifier = "pitcher_crop",
    replacement_variants = [int_prop "age" 0 4, enum_prop "half" ["upper", "lower"]],
    default_override =
      make_kv_list { age = "0", half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "pitcher_plant",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  transparent_reg("beetroots"),
  transparent_reg("dirt_path"),
  basic_light_reg("end_gateway", 15),
  {
    identifier = "repeating_command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  {
    identifier = "chain_command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  basic_reg("frosted_ice"),
  basic_light_reg("magma_block", 3),
  basic_reg("nether_wart_block"),
  basic_reg("red_nether_bricks"),
  {
    identifier = "bone_block",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  transparent_reg("structure_void"),
  {
    identifier = "observer",
    replacement_variants = [facing_neswud, powered],
    default_override =
      make_kv_list { facing = "south", powered = "false" },
  } | StandardRegistration,
  shulker_box_reg("shulker_box"),
  shulker_box_reg("white_shulker_box"),
  shulker_box_reg("orange_shulker_box"),
  shulker_box_reg("magenta_shulker_box"),
  shulker_box_reg("light_blue_shulker_box"),
  shulker_box_reg("yellow_shulker_box"),
  shulker_box_reg("lime_shulker_box"),
  shulker_box_reg("pink_shulker_box"),
  shulker_box_reg("gray_shulker_box"),
  shulker_box_reg("light_gray_shulker_box"),
  shulker_box_reg("cyan_shulker_box"),
  shulker_box_reg("purple_shulker_box"),
  shulker_box_reg("blue_shulker_box"),
  shulker_box_reg("brown_shulker_box"),
  shulker_box_reg("green_shulker_box"),
  shulker_box_reg("red_shulker_box"),
  shulker_box_reg("black_shulker_box"),
  glazed_terracotta_reg("white_glazed_terracotta"),
  glazed_terracotta_reg("orange_glazed_terracotta"),
  glazed_terracotta_reg("magenta_glazed_terracotta"),
  glazed_terracotta_reg("light_blue_glazed_terracotta"),
  glazed_terracotta_reg("yellow_glazed_terracotta"),
  glazed_terracotta_reg("lime_glazed_terracotta"),
  glazed_terracotta_reg("pink_glazed_terracotta"),
  glazed_terracotta_reg("gray_glazed_terracotta"),
  glazed_terracotta_reg("light_gray_glazed_terracotta"),
  glazed_terracotta_reg("cyan_glazed_terracotta"),
  glazed_terracotta_reg("purple_glazed_terracotta"),
  glazed_terracotta_reg("blue_glazed_terracotta"),
  glazed_terracotta_reg("brown_glazed_terracotta"),
  glazed_terracotta_reg("green_glazed_terracotta"),
  glazed_terracotta_reg("red_glazed_terracotta"),
  glazed_terracotta_reg("black_glazed_terracotta"),
  basic_reg("white_concrete"),
  basic_reg("orange_concrete"),
  basic_reg("magenta_concrete"),
  basic_reg("light_blue_concrete"),
  basic_reg("yellow_concrete"),
  basic_reg("lime_concrete"),
  basic_reg("pink_concrete"),
  basic_reg("gray_concrete"),
  basic_reg("light_gray_concrete"),
  basic_reg("cyan_concrete"),
  basic_reg("purple_concrete"),
  basic_reg("blue_concrete"),
  basic_reg("brown_concrete"),
  basic_reg("green_concrete"),
  basic_reg("red_concrete"),
  basic_reg("black_concrete"),
  basic_reg("white_concrete_powder"),
  basic_reg("orange_concrete_powder"),
  basic_reg("magenta_concrete_powder"),
  basic_reg("light_blue_concrete_powder"),
  basic_reg("yellow_concrete_powder"),
  basic_reg("lime_concrete_powder"),
  basic_reg("pink_concrete_powder"),
  basic_reg("gray_concrete_powder"),
  basic_reg("light_gray_concrete_powder"),
  basic_reg("cyan_concrete_powder"),
  basic_reg("purple_concrete_powder"),
  basic_reg("blue_concrete_powder"),
  basic_reg("brown_concrete_powder"),
  basic_reg("green_concrete_powder"),
  basic_reg("red_concrete_powder"),
  basic_reg("black_concrete_powder"),
  {
    identifier = "kelp",
    custom_variants = [int_prop "age" 0 25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_no_collider_reg("kelp_plant"),
  basic_reg("dried_kelp_block"),
  transparent_reg("turtle_egg"),
  transparent_reg("sniffer_egg"),
  basic_reg("dead_tube_coral_block"),
  basic_reg("dead_brain_coral_block"),
  basic_reg("dead_bubble_coral_block"),
  basic_reg("dead_fire_coral_block"),
  basic_reg("dead_horn_coral_block"),
  basic_reg("tube_coral_block"),
  basic_reg("brain_coral_block"),
  basic_reg("bubble_coral_block"),
  basic_reg("fire_coral_block"),
  basic_reg("horn_coral_block"),
  coral_reg("dead_tube_coral"),
  coral_reg("dead_brain_coral"),
  coral_reg("dead_bubble_coral"),
  coral_reg("dead_fire_coral"),
  coral_reg("dead_horn_coral"),
  coral_reg("tube_coral"),
  coral_reg("brain_coral"),
  coral_reg("bubble_coral"),
  coral_reg("fire_coral"),
  coral_reg("horn_coral"),
  coral_reg("dead_tube_coral_fan"),
  coral_reg("dead_brain_coral_fan"),
  coral_reg("dead_bubble_coral_fan"),
  coral_reg("dead_fire_coral_fan"),
  coral_reg("dead_horn_coral_fan"),
  coral_reg("tube_coral_fan"),
  coral_reg("brain_coral_fan"),
  coral_reg("bubble_coral_fan"),
  coral_reg("fire_coral_fan"),
  coral_reg("horn_coral_fan"),
  coral_wall_fan_reg("dead_tube_coral_wall_fan"),
  coral_wall_fan_reg("dead_brain_coral_wall_fan"),
  coral_wall_fan_reg("dead_bubble_coral_wall_fan"),
  coral_wall_fan_reg("dead_fire_coral_wall_fan"),
  coral_wall_fan_reg("dead_horn_coral_wall_fan"),
  coral_wall_fan_reg("tube_coral_wall_fan"),
  coral_wall_fan_reg("brain_coral_wall_fan"),
  coral_wall_fan_reg("bubble_coral_wall_fan"),
  coral_wall_fan_reg("fire_coral_wall_fan"),
  coral_wall_fan_reg("horn_coral_wall_fan"),
  {
    identifier = "sea_pickle",
    replacement_variants = [int_prop "pickles" 1 4, bool_prop "waterlogged"],
    default_override =
      make_kv_list { pickles = "1", waterlogged = "true" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { pickles = "4", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 12,
          },
        },
        conditions = make_kv_list { pickles = "3", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 9
          },
        },
        conditions = make_kv_list { pickles = "2", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 6
          },
        },
        conditions = make_kv_list { pickles = "1", waterlogged = "true" },
      },
    ],
  } | StandardRegistration,
  basic_reg("blue_ice"),
  {
    identifier = "conduit",
    custom_variants = [waterlogged],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  transparent_reg("bamboo_sapling"),
  {
    identifier = "bamboo",
    custom_variants = [
      int_prop "age" 0 1,
      enum_prop "leaves" ["none", "small", "large"],
      int_prop "stage" 0 1,
    ],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("potted_bamboo"),
  {
    identifier = "void_air",
    properties = { air_like = true },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "cave_air",
    properties = { air_like = true },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "bubble_column",
    custom_variants = [bool_prop "drag"],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  stairs_reg("polished_granite_stairs"),
  stairs_reg("smooth_red_sandstone_stairs"),
  stairs_reg("mossy_stone_brick_stairs"),
  stairs_reg("polished_diorite_stairs"),
  stairs_reg("mossy_cobblestone_stairs"),
  stairs_reg("end_stone_brick_stairs"),
  stairs_reg("stone_stairs"),
  stairs_reg("smooth_sandstone_stairs"),
  stairs_reg("smooth_quartz_stairs"),
  stairs_reg("granite_stairs"),
  stairs_reg("andesite_stairs"),
  stairs_reg("red_nether_brick_stairs"),
  stairs_reg("polished_andesite_stairs"),
  stairs_reg("diorite_stairs"),
  slab_reg("polished_granite_slab"),
  slab_reg("smooth_red_sandstone_slab"),
  slab_reg("mossy_stone_brick_slab"),
  slab_reg("polished_diorite_slab"),
  slab_reg("mossy_cobblestone_slab"),
  slab_reg("end_stone_brick_slab"),
  slab_reg("smooth_sandstone_slab"),
  slab_reg("smooth_quartz_slab"),
  slab_reg("granite_slab"),
  slab_reg("andesite_slab"),
  slab_reg("red_nether_brick_slab"),
  slab_reg("polished_andesite_slab"),
  slab_reg("diorite_slab"),
  wall_reg("brick_wall"),
  wall_reg("prismarine_wall"),
  wall_reg("red_sandstone_wall"),
  wall_reg("mossy_stone_brick_wall"),
  wall_reg("granite_wall"),
  wall_reg("stone_brick_wall"),
  wall_reg("mud_brick_wall"),
  wall_reg("nether_brick_wall"),
  wall_reg("andesite_wall"),
  wall_reg("red_nether_brick_wall"),
  wall_reg("sandstone_wall"),
  wall_reg("end_stone_brick_wall"),
  wall_reg("diorite_wall"),
  {
    identifier = "scaffolding",
    custom_variants = [int_prop "distance" 0 7, waterlogged],
    replacement_variants = [bool_prop "bottom"],
    default_override =
      make_kv_list { bottom = "false", distance = "7", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "loom",
    replacement_variants = [facing_nswe],
  } | StandardRegistration,
  {
    identifier = "barrel",
    replacement_variants = [facing_neswud, bool_prop "open"],
    default_override =
      make_kv_list { facing = "north", open = "false" },
  } | StandardRegistration,
  {
    identifier = "smoker",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "blast_furnace",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  basic_reg("cartography_table"),
  basic_reg("fletching_table"),
  {
    identifier = "grindstone",
    replacement_variants = [enum_prop "face" ["floor", "wall", "ceiling"], facing_nswe],
    default_override =
      make_kv_list { face = "wall", facing = "north" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "lectern",
    custom_variants = [bool_prop "has_book", powered],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", has_book = "false", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("smithing_table"),
  {
    identifier = "stonecutter",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "bell",
    custom_variants = [powered],
    replacement_variants = [
      enum_prop
        "attachment"
        ["floor", "ceiling", "single_wall", "double_wall"],
      facing_nswe,
    ],
    default_override =
      make_kv_list {
        attachment = "floor",
        facing = "north",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  lantern_reg("lantern", 15),
  lantern_reg("soul_lantern", 10),
  {
    identifier = "campfire",
    custom_variants = [bool_prop "signal_fire", waterlogged],
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list {
        facing = "north",
        lit = "true",
        signal_fire = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "soul_campfire",
    custom_variants = [bool_prop "signal_fire", waterlogged],
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list {
        facing = "north",
        lit = "true",
        signal_fire = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 10,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  transparent_no_collider_reg("sweet_berry_bush"),
  {
    identifier = "warped_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_warped_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "warped_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_warped_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  basic_reg("warped_nylium"),
  basic_reg("warped_fungus"),
  basic_reg("warped_wart_block"),
  basic_reg("warped_roots"),
  basic_reg("nether_sprouts"),
  {
    identifier = "crimson_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_crimson_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "crimson_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_crimson_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  basic_reg("crimson_nylium"),
  basic_reg("crimson_fungus"),
  basic_light_reg("shroomlight", 15),
  {
    identifier = "weeping_vines",
    custom_variants = [age_0_25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("weeping_vines_plant"),
  {
    identifier = "twisting_vines",
    custom_variants = [age_0_25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("twisting_vines_plant"),
  basic_reg("crimson_roots"),
  basic_reg("crimson_planks"),
  basic_reg("warped_planks"),
  slab_reg("crimson_slab"),
  slab_reg("warped_slab"),
  pressure_plate_reg("crimson_pressure_plate"),
  pressure_plate_reg("warped_pressure_plate"),
  fence_reg("crimson_fence"),
  fence_reg("warped_fence"),
  trapdoor_reg("crimson_trapdoor"),
  trapdoor_reg("warped_trapdoor"),
  fence_gate_reg("crimson_fence_gate"),
  fence_gate_reg("warped_fence_gate"),
  stairs_reg("crimson_stairs"),
  stairs_reg("warped_stairs"),
  button_reg("crimson_button"),
  button_reg("warped_button"),
  door_reg("crimson_door"),
  door_reg("warped_door"),
  sign_reg("crimson_sign"),
  sign_reg("warped_sign"),
  wall_sign_reg("crimson_wall_sign"),
  wall_sign_reg("warped_wall_sign"),
  {
    identifier = "structure_block",
    replacement_variants = [enum_prop "mode" ["save", "load", "corner", "data"]],
    default_override = make_kv_list { mode = "load" },
  } | StandardRegistration,
  {
    identifier = "jigsaw",
    replacement_variants = [
      enum_prop
        "orientation"
        [
          "down_east",
          "down_north",
          "down_south",
          "down_west",
          "up_east",
          "up_north",
          "up_south",
          "up_west",
          "west_up",
          "east_up",
          "north_up",
          "south_up",
        ],
    ],
    default_override =
      make_kv_list { orientation = "north_up" },
  } | StandardRegistration,
  {
    identifier = "composter",
    custom_variants = [int_prop "level" 0 8],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "target",
    custom_variants = [int_prop "power" 0 15],
  } | StandardRegistration,
  {
    identifier = "bee_nest",
    replacement_variants = [facing_nswe, int_prop "honey_level" 0 5],
  } | StandardRegistration,
  {
    identifier = "beehive",
    replacement_variants = [facing_nswe, int_prop "honey_level" 0 5],
  } | StandardRegistration,
  transparent_reg("honey_block"),
  basic_reg("honeycomb_block"),
  basic_reg("netherite_block"),
  basic_reg("ancient_debris"),
  basic_light_reg("crying_obsidian", 10),
  {
    identifier = "respawn_anchor",
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { charges = "4" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 11,
          },
        },
        conditions = make_kv_list { charges = "3" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 7
          },
        },
        conditions = make_kv_list { charges = "2" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 3
          },
        },
        conditions = make_kv_list { charges = "1" },
      },
    ],
  } | StandardRegistration,
  transparent_reg("potted_crimson_fungus"),
  transparent_reg("potted_warped_fungus"),
  transparent_reg("potted_crimson_roots"),
  transparent_reg("potted_warped_roots"),
  basic_reg("lodestone"),
  basic_reg("blackstone"),
  stairs_reg("blackstone_stairs"),
  wall_reg("blackstone_wall"),
  slab_reg("blackstone_slab"),
  basic_reg("polished_blackstone"),
  basic_reg("polished_blackstone_bricks"),
  basic_reg("cracked_polished_blackstone_bricks"),
  basic_reg("chiseled_polished_blackstone"),
  slab_reg("polished_blackstone_brick_slab"),
  stairs_reg("polished_blackstone_brick_stairs"),
  wall_reg("polished_blackstone_brick_wall"),
  basic_reg("gilded_blackstone"),
  stairs_reg("polished_blackstone_stairs"),
  slab_reg("polished_blackstone_slab"),
  pressure_plate_reg("polished_blackstone_pressure_plate"),
  button_reg("polished_blackstone_button"),
  wall_reg("polished_blackstone_wall"),
  basic_reg("chiseled_nether_bricks"),
  basic_reg("cracked_nether_bricks"),
  basic_reg("quartz_bricks"),
  candle_reg("candle"),
  candle_reg("white_candle"),
  candle_reg("orange_candle"),
  candle_reg("magenta_candle"),
  candle_reg("light_blue_candle"),
  candle_reg("yellow_candle"),
  candle_reg("lime_candle"),
  candle_reg("pink_candle"),
  candle_reg("gray_candle"),
  candle_reg("light_gray_candle"),
  candle_reg("cyan_candle"),
  candle_reg("purple_candle"),
  candle_reg("blue_candle"),
  candle_reg("brown_candle"),
  candle_reg("green_candle"),
  candle_reg("red_candle"),
  candle_reg("black_candle"),
  candle_cake_reg("candle_cake"),
  candle_cake_reg("white_candle_cake"),
  candle_cake_reg("orange_candle_cake"),
  candle_cake_reg("magenta_candle_cake"),
  candle_cake_reg("light_blue_candle_cake"),
  candle_cake_reg("yellow_candle_cake"),
  candle_cake_reg("lime_candle_cake"),
  candle_cake_reg("pink_candle_cake"),
  candle_cake_reg("gray_candle_cake"),
  candle_cake_reg("light_gray_candle_cake"),
  candle_cake_reg("cyan_candle_cake"),
  candle_cake_reg("purple_candle_cake"),
  candle_cake_reg("blue_candle_cake"),
  candle_cake_reg("brown_candle_cake"),
  candle_cake_reg("green_candle_cake"),
  candle_cake_reg("red_candle_cake"),
  candle_cake_reg("black_candle_cake"),
  basic_reg("amethyst_block"),
  basic_reg("budding_amethyst"),
  {
    identifier = "amethyst_cluster",
    custom_variants = [waterlogged],
    replacement_variants = [facing_neswud],
    default_override =
      make_kv_list { facing = "up", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 5
      },
    },
  } | StandardRegistration,
  amethyst_bud_reg("large_amethyst_bud", 5),
  amethyst_bud_reg("medium_amethyst_bud", 2),
  amethyst_bud_reg("small_amethyst_bud", 1),
  basic_reg("tuff"),
  slab_reg("tuff_slab"),
  stairs_reg("tuff_stairs"),
  wall_reg("tuff_wall"),
  basic_reg("polished_tuff"),
  slab_reg("polished_tuff_slab"),
  stairs_reg("polished_tuff_stairs"),
  wall_reg("polished_tuff_wall"),
  basic_reg("chiseled_tuff"),
  basic_reg("tuff_bricks"),
  slab_reg("tuff_brick_slab"),
  stairs_reg("tuff_brick_stairs"),
  wall_reg("tuff_brick_wall"),
  basic_reg("chiseled_tuff_bricks"),
  basic_reg("calcite"),
  transparent_reg("tinted_glass"),
  basic_reg("powder_snow"),
  {
    identifier = "sculk_sensor",
    custom_variants = [
      int_prop "power" 0 15,
      enum_prop "sculk_sensor_phase" ["inactive", "active", "cooldown"],
      waterlogged,
    ],
    skip_properties = ["power", "waterlogged"],
    default_override =
      make_kv_list {
        power = "0",
        sculk_sensor_phase = "inactive",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | FullCustomRegistration,
  {
    identifier = "calibrated_sculk_sensor",
    custom_variants = [
      facing_nswe,
      int_prop "power" 0 15,
      enum_prop "sculk_sensor_phase" ["inactive", "active", "cooldown"],
      waterlogged,
    ],
    skip_properties = ["power", "waterlogged"],
    default_override =
      make_kv_list {
        facing = "north",
        power = "0",
        sculk_sensor_phase = "inactive",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  basic_reg("sculk"),
  {
    identifier = "sculk_vein",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        down = "false",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "sculk_catalyst",
    replacement_variants = [bool_prop "bloom"],
    default_override = make_kv_list { bloom = "false" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 6
      },
    },
  } | StandardRegistration,
  {
    identifier = "sculk_shrieker",
    custom_variants = [bool_prop "shrieking", waterlogged],
    replacement_variants = [bool_prop "can_summon"],
    default_override =
      make_kv_list {
        can_summon = "false",
        shrieking = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("copper_block"),
  basic_reg("exposed_copper"),
  basic_reg("weathered_copper"),
  basic_reg("oxidized_copper"),
  basic_reg("copper_ore"),
  basic_reg("deepslate_copper_ore"),
  basic_reg("oxidized_cut_copper"),
  basic_reg("weathered_cut_copper"),
  basic_reg("exposed_cut_copper"),
  basic_reg("cut_copper"),
  basic_reg("oxidized_chiseled_copper"),
  basic_reg("weathered_chiseled_copper"),
  basic_reg("exposed_chiseled_copper"),
  basic_reg("chiseled_copper"),
  basic_reg("waxed_oxidized_chiseled_copper"),
  basic_reg("waxed_weathered_chiseled_copper"),
  basic_reg("waxed_exposed_chiseled_copper"),
  basic_reg("waxed_chiseled_copper"),
  stairs_reg("oxidized_cut_copper_stairs"),
  stairs_reg("weathered_cut_copper_stairs"),
  stairs_reg("exposed_cut_copper_stairs"),
  stairs_reg("cut_copper_stairs"),
  slab_reg("oxidized_cut_copper_slab"),
  slab_reg("weathered_cut_copper_slab"),
  slab_reg("exposed_cut_copper_slab"),
  slab_reg("cut_copper_slab"),
  basic_reg("waxed_copper_block"),
  basic_reg("waxed_weathered_copper"),
  basic_reg("waxed_exposed_copper"),
  basic_reg("waxed_oxidized_copper"),
  basic_reg("waxed_oxidized_cut_copper"),
  basic_reg("waxed_weathered_cut_copper"),
  basic_reg("waxed_exposed_cut_copper"),
  basic_reg("waxed_cut_copper"),
  stairs_reg("waxed_oxidized_cut_copper_stairs"),
  stairs_reg("waxed_weathered_cut_copper_stairs"),
  stairs_reg("waxed_exposed_cut_copper_stairs"),
  stairs_reg("waxed_cut_copper_stairs"),
  slab_reg("waxed_oxidized_cut_copper_slab"),
  slab_reg("waxed_weathered_cut_copper_slab"),
  slab_reg("waxed_exposed_cut_copper_slab"),
  slab_reg("waxed_cut_copper_slab"),
  door_reg("copper_door"),
  door_reg("exposed_copper_door"),
  door_reg("oxidized_copper_door"),
  door_reg("weathered_copper_door"),
  door_reg("waxed_copper_door"),
  door_reg("waxed_exposed_copper_door"),
  door_reg("waxed_oxidized_copper_door"),
  door_reg("waxed_weathered_copper_door"),
  trapdoor_reg("copper_trapdoor"),
  trapdoor_reg("exposed_copper_trapdoor"),
  trapdoor_reg("oxidized_copper_trapdoor"),
  trapdoor_reg("weathered_copper_trapdoor"),
  trapdoor_reg("waxed_copper_trapdoor"),
  trapdoor_reg("waxed_exposed_copper_trapdoor"),
  trapdoor_reg("waxed_oxidized_copper_trapdoor"),
  trapdoor_reg("waxed_weathered_copper_trapdoor"),
  grate_reg("copper_grate"),
  grate_reg("exposed_copper_grate"),
  grate_reg("weathered_copper_grate"),
  grate_reg("oxidized_copper_grate"),
  grate_reg("waxed_copper_grate"),
  grate_reg("waxed_exposed_copper_grate"),
  grate_reg("waxed_weathered_copper_grate"),
  grate_reg("waxed_oxidized_copper_grate"),
  bulb_reg("copper_bulb", 15),
  bulb_reg("exposed_copper_bulb", 12),
  bulb_reg("weathered_copper_bulb", 8),
  bulb_reg("oxidized_copper_bulb", 4),
  bulb_reg("waxed_copper_bulb", 15),
  bulb_reg("waxed_exposed_copper_bulb", 12),
  bulb_reg("waxed_weathered_copper_bulb", 8),
  bulb_reg("waxed_oxidized_copper_bulb", 4),
  {
    identifier = "lightning_rod",
    custom_variants = [waterlogged],
    replacement_variants = [facing_neswud, powered],
    default_override =
      make_kv_list { facing = "up", powered = "false", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "pointed_dripstone",
    custom_variants = [waterlogged],
    replacement_variants = [
      enum_prop
        "thickness"
        ["tip_merge", "tip", "frustum", "middle", "base"],
      enum_prop "vertical_direction" ["up", "down"],
    ],
    default_override =
      make_kv_list {
        thickness = "tip",
        vertical_direction = "up",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  basic_reg("dripstone_block"),
  {
    identifier = "cave_vines",
    custom_variants = [age_0_25, bool_prop "berries"],
    skip_properties = ["age"],
    default_override =
      make_kv_list { age = "0", berries = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 14,
          },
        },
        conditions = make_kv_list { berries = "true" },
      },
    ],
  } | FullCustomRegistration,
  {
    identifier = "cave_vines_plant",
    replacement_variants = [bool_prop "berries"],
    default_override = make_kv_list { berries = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("spore_blossom"),
  transparent_reg("azalea"),
  transparent_reg("flowering_azalea"),
  transparent_reg("moss_carpet"),
  {
    identifier = "pink_petals",
    custom_variants = [facing_nswe, int_prop "flower_amount" 1 4],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  transparent_reg("moss_block"),
  {
    identifier = "big_dripleaf",
    custom_variants = [waterlogged],
    replacement_variants = [
      facing_nswe,
      enum_prop "tilt" ["none", "unstable", "partial", "full"],
    ],
    default_override =
      make_kv_list { facing = "north", tilt = "none", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "big_dripleaf_stem",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "small_dripleaf",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe, enum_prop "half" ["upper", "lower"]],
    default_override =
      make_kv_list { facing = "north", half = "lower", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "hanging_roots",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
  } | StandardRegistration,
  basic_reg("rooted_dirt"),
  basic_reg("mud"),
  {
    identifier = "deepslate",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  basic_reg("cobbled_deepslate"),
  stairs_reg("cobbled_deepslate_stairs"),
  slab_reg("cobbled_deepslate_slab"),
  wall_reg("cobbled_deepslate_wall"),
  basic_reg("polished_deepslate"),
  stairs_reg("polished_deepslate_stairs"),
  slab_reg("polished_deepslate_slab"),
  wall_reg("polished_deepslate_wall"),
  basic_reg("deepslate_tiles"),
  stairs_reg("deepslate_tile_stairs"),
  slab_reg("deepslate_tile_slab"),
  wall_reg("deepslate_tile_wall"),
  basic_reg("deepslate_bricks"),
  stairs_reg("deepslate_brick_stairs"),
  slab_reg("deepslate_brick_slab"),
  wall_reg("deepslate_brick_wall"),
  basic_reg("chiseled_deepslate"),
  basic_reg("cracked_deepslate_bricks"),
  basic_reg("cracked_deepslate_tiles"),
  {
    identifier = "infested_deepslate",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  basic_reg("smooth_basalt"),
  basic_reg("raw_iron_block"),
  basic_reg("raw_copper_block"),
  basic_reg("raw_gold_block"),
  transparent_reg("potted_azalea_bush"),
  transparent_reg("potted_flowering_azalea_bush"),
  {
    identifier = "ochre_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  {
    identifier = "verdant_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  {
    identifier = "pearlescent_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  transparent_reg("frogspawn"),
  basic_reg("reinforced_deepslate"),
  {
    identifier = "decorated_pot",
    custom_variants = [bool_prop "cracked", facing_nswe, waterlogged],
    default_override =
      make_kv_list {
        cracked = "false",
        facing = "north",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "crafter",
    replacement_variants = [
      bool_prop "crafting",
      enum_prop
        "orientation"
        [
          "down_east",
          "down_north",
          "down_south",
          "down_west",
          "up_east",
          "up_north",
          "up_south",
          "up_west",
          "west_up",
          "east_up",
          "north_up",
          "south_up",
        ],
      bool_prop "triggered",
    ],
    default_override =
      make_kv_list {
        crafting = "false",
        orientation = "north_up",
        triggered = "false",
      },
  } | StandardRegistration,
  {
    identifier = "trial_spawner",
    replacement_variants = [
      bool_prop "ominous",
      enum_prop
        "trial_spawner_state"
        [
          "inactive",
          "waiting_for_players",
          "active",
          "waiting_for_reward_ejection",
          "ejecting_reward",
          "cooldown",
        ],
    ],
    default_override =
      make_kv_list { ominous = "false", trial_spawner_state = "inactive" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "vault",
    replacement_variants = [
      facing_nswe,
      bool_prop "ominous",
      enum_prop
        "vault_state"
        ["inactive", "active", "unlocking", "ejecting"],
    ],
    default_override =
      make_kv_list {
        facing = "north",
        ominous = "false",
        vault_state = "inactive",
      },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "heavy_core",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  },
    ]
}
