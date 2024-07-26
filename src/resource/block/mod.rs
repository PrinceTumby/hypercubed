pub mod blockstate;
pub mod model;

use super::{texture, Identifier, RegistryData, RegistryIndex};
use crate::client::graphics::chunk::block_face::rotation_matrices;
use ahash::AHashMap;
use anyhow::Context;
use blockstate::CustomPropertyType;
use model::ModelCache;
use serde_repr::Deserialize_repr;
use string_cache::DefaultAtom as Atom;

#[derive(Debug)]
pub struct Registry {
    data: RegistryData<Info>,
    pub global_palette: Vec<blockstate::Blockstate>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalPaletteIndex(u16);

impl GlobalPaletteIndex {
    pub fn placeholder() -> Self {
        Self(0xFFFF)
    }

    pub fn as_raw(&self) -> u16 {
        self.0
    }
}

impl Default for GlobalPaletteIndex {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl From<GlobalPaletteIndex> for usize {
    fn from(value: GlobalPaletteIndex) -> Self {
        value.as_usize()
    }
}

macro_rules! impl_palette_index_try_from {
    ($from_type:ty) => {
        impl TryFrom<$from_type> for GlobalPaletteIndex {
            type Error = <u16 as TryFrom<$from_type>>::Error;

            fn try_from(value: $from_type) -> Result<Self, Self::Error> {
                u16::try_from(value).map(Self)
            }
        }
    };
}

impl_palette_index_try_from!(u32);
impl_palette_index_try_from!(u64);
impl_palette_index_try_from!(usize);
impl_palette_index_try_from!(i32);
impl_palette_index_try_from!(i64);
impl_palette_index_try_from!(isize);

impl GlobalPaletteIndex {
    #[inline(always)]
    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            data: RegistryData::new(),
            global_palette: Vec::new(),
        }
    }

    /// Panics if an entry is already registered with `identifier`.
    pub fn register(
        &mut self,
        identifier: Identifier,
        custom_variant_properties: Option<&[(&str, CustomPropertyType)]>,
        replacement_variant_properties: Option<&[(&str, CustomPropertyType)]>,
        default_override: Option<&[(&str, &str)]>,
        properties: Properties,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<RegistryIndex> {
        let block_index = self.data.register_default(identifier.clone());
        let mut blockstates = blockstate::load_blockstates(
            block_index,
            &identifier,
            custom_variant_properties,
            replacement_variant_properties,
            model_cache,
            texture_atlas,
        )
        .with_context(|| format!("Failed to parse blockstates for {identifier:?}"))?;
        #[cfg(debug_assertions)]
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override
                    .into_iter()
                    .map(|(k, v)| (Atom::from(*k), Atom::from(*v)))
                    .collect();
                self.global_palette.len()
                    + blockstates
                        .iter()
                        .position(|blockstate| blockstate.properties == override_map)
                        .expect("`default_override` should exist as a valid state")
            }
            None => self.global_palette.len(),
        };
        self.global_palette.append(&mut blockstates);
        self.data[block_index] = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range,
        };
        Ok(block_index)
    }

    /// Panics if an entry is already registered with `identifier`.
    /// `custom_properties` is each property that defines the blockstates, in order.
    /// `skip_properties` is a list of property names from `properties` that do not appear in the
    /// blockstates file.
    pub fn register_full_custom(
        &mut self,
        identifier: Identifier,
        custom_properties: &[(&str, CustomPropertyType)],
        skip_properties: &[&str],
        default_override: Option<&[(&str, &str)]>,
        properties: Properties,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<RegistryIndex> {
        let block_index = self.data.register_default(identifier.clone());
        let mut blockstates = blockstate::load_full_custom_blockstates(
            block_index,
            &identifier,
            custom_properties,
            skip_properties,
            model_cache,
            texture_atlas,
        )
        .with_context(|| format!("Failed to parse blockstates for {identifier:?}"))?;
        #[cfg(debug_assertions)]
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override
                    .into_iter()
                    .map(|(k, v)| (Atom::from(*k), Atom::from(*v)))
                    .collect();
                self.global_palette.len()
                    + blockstates
                        .iter()
                        .position(|blockstate| blockstate.properties == override_map)
                        .expect("`default_override` should exist as a valid state")
            }
            None => self.global_palette.len(),
        };
        self.global_palette.append(&mut blockstates);
        self.data[block_index] = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range,
        };
        Ok(block_index)
    }

    pub fn register_liquid(
        &mut self,
        identifier: Identifier,
        mut properties: Properties,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<RegistryIndex> {
        let block_index = self.data.register_default(identifier.clone());
        properties.opaque = false;
        let mut blockstates =
            blockstate::load_liquid_blockstates(block_index, &identifier, model_cache, texture_atlas)
                .with_context(|| format!("while parsing liquid blockstates for {identifier:?}"))?;
        #[cfg(debug_assertions)]
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = self.global_palette.len();
        self.global_palette.append(&mut blockstates);
        self.data[block_index] = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range,
        };
        Ok(block_index)
    }

    pub fn get_index_from_identifier(&self, identifier: &Identifier) -> Option<RegistryIndex> {
        self.data.get_index_from_identifier(identifier)
    }

    pub fn get_entry_from_identifier(&self, identifier: &Identifier) -> Option<&Info> {
        self.data.get_entry_from_identifier(identifier)
    }

    pub fn get_identifier_from_index(&self, index: RegistryIndex) -> Option<&Identifier> {
        self.data.get_identifier_from_index(index)
    }
}

impl std::ops::Index<RegistryIndex> for Registry {
    type Output = Info;

    fn index(&self, index: RegistryIndex) -> &Self::Output {
        &self.data[index]
    }
}

impl std::ops::IndexMut<RegistryIndex> for Registry {
    fn index_mut(&mut self, index: RegistryIndex) -> &mut Self::Output {
        &mut self.data[index]
    }
}

#[derive(Debug)]
pub struct Info {
    pub default_blockstate: GlobalPaletteIndex,
    pub properties: Properties,
    #[cfg(debug_assertions)]
    pub blockstate_id_range: std::ops::RangeInclusive<usize>,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            default_blockstate: GlobalPaletteIndex::placeholder(),
            properties: Properties::default(),
            #[cfg(debug_assertions)]
            blockstate_id_range: usize::MAX..=usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Properties {
    pub opaque: bool,
}

impl Default for Properties {
    fn default() -> Self {
        Self { opaque: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize_repr)]
#[repr(u16)]
pub enum RightAngleRotation {
    #[default]
    Zero = 0,
    Ninety = 90,
    OneEighty = 180,
    TwoSeventy = 270,
}

impl RightAngleRotation {
    pub fn matrix_index(&self) -> u8 {
        match self {
            &RightAngleRotation::Zero => rotation_matrices::indices::ZERO,
            &RightAngleRotation::Ninety => rotation_matrices::indices::NINETY,
            &RightAngleRotation::OneEighty => rotation_matrices::indices::ONE_EIGHTY,
            &RightAngleRotation::TwoSeventy => rotation_matrices::indices::TWO_SEVENTY,
        }
    }
}

// TODO Make model loading errors print out a warning and load missing texture block instead

pub fn register_vanilla_blocks(
    registry: &mut Registry,
    model_cache: &mut ModelCache,
    texture_atlas_builder: &mut texture::AtlasBuilder,
) -> anyhow::Result<()> {
    use CustomPropertyType::*;
    macro_rules! register {
        // Custom block properties
        ($identifier:expr, $( $key:ident = $value:expr ),+) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                None,
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                None,
                Some($default_override),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $custom_variants:expr, $( $key:ident = $value:expr ),+) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                None,
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            $custom_variants:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                None,
                Some($default_override),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            replace: $replacement_variants:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Some($replacement_variants),
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            replace: $replacement_variants:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Some($replacement_variants),
                Some($default_override),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            replace: $replacement_variants:expr,
            $custom_variants:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Some($replacement_variants),
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            replace: $replacement_variants:expr,
            $custom_variants:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Some($replacement_variants),
                Some($default_override),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            full_custom: $properties:expr,
            skips: $skips:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register_full_custom(
                Identifier::parse($identifier).unwrap(),
                $properties,
                $skips,
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            full_custom: $properties:expr,
            skips: $skips:expr,
            $( $key:ident = $value:expr ),+
        ) => {
            registry.register_full_custom(
                Identifier::parse($identifier).unwrap(),
                $properties,
                $skips,
                Some($default_override),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        // Default block properties
        ($identifier:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                None,
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, override_default: $default_override:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                None,
                Some($default_override),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $custom_variants:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                None,
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, override_default: $default_override:expr, $custom_variants:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                None,
                Some($default_override),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, replace: $replacement_variants:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Some($replacement_variants),
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            replace: $replacement_variants:expr
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Some($replacement_variants),
                Some($default_override),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, replace: $replacement_variants:expr, $custom_variants:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Some($replacement_variants),
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            replace: $replacement_variants:expr,
            $custom_variants:expr
        ) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Some($replacement_variants),
                Some($default_override),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, full_custom: $properties:expr, skips: $skips:expr) => {
            registry.register_full_custom(
                Identifier::parse($identifier).unwrap(),
                $properties,
                $skips,
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        (
            $identifier:expr,
            override_default: $default_override:expr,
            full_custom: $properties:expr,
            skips: $skips:expr
        ) => {
            registry.register_full_custom(
                Identifier::parse($identifier).unwrap(),
                $properties,
                $skips,
                Some($default_override),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
    }
    macro_rules! register_liquid {
        ($identifier:expr) => {
            registry.register_liquid(
                Identifier::parse($identifier).unwrap(),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $( $key:ident = $value:expr ),+) => {
            registry.register_liquid(
                Identifier::parse($identifier).unwrap(),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
    }

    // Common properties
    const FACING_NSWE: (&str, blockstate::CustomPropertyType) =
        ("facing", Enum(&["north", "south", "west", "east"]));
    const FACING_NESWUD: (&str, blockstate::CustomPropertyType) = (
        "facing",
        Enum(&["north", "east", "south", "west", "up", "down"]),
    );
    const WATERLOGGED: (&str, blockstate::CustomPropertyType) = ("waterlogged", Bool);
    const POWERED: (&str, blockstate::CustomPropertyType) = ("powered", Bool);
    const LIT: (&str, blockstate::CustomPropertyType) = ("lit", Bool);
    const AGE_0_15: (&str, blockstate::CustomPropertyType) = ("age", Int(0..=15));
    const AGE_0_25: (&str, blockstate::CustomPropertyType) = ("age", Int(0..=25));
    const ROTATION_0_15: (&str, blockstate::CustomPropertyType) = ("rotation", Int(0..=15));
    const CHEST_TYPE: (&str, blockstate::CustomPropertyType) =
        ("type", Enum(&["single", "left", "right"]));

    let start_time = std::time::Instant::now();

    // Specialised registration macros for common block types
    macro_rules! register_slab {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("type", "bottom"), ("waterlogged", "false")],
                replace: &[("type", Enum(&["top", "bottom", "double"]))],
                &[WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_stairs {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false")
                ],
                replace: &[
                    FACING_NSWE,
                    ("half", Enum(&["top", "bottom"])),
                    ("shape", Enum(&[
                        "straight",
                        "inner_left",
                        "inner_right",
                        "outer_left",
                        "outer_right",
                    ])),
                ],
                &[WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_fence {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("east", "false"),
                    ("north", "false"),
                    ("south", "false"),
                    ("west", "false"),
                    ("waterlogged", "false"),
                ],
                &[
                    ("east", Bool),
                    ("north", Bool),
                    ("south", Bool),
                    WATERLOGGED,
                    ("west", Bool),
                ],
                opaque = false
            )
        };
    }
    macro_rules! register_fence_gate {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("facing", "north"),
                    ("in_wall", "false"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
                replace: &[
                    FACING_NSWE,
                    ("in_wall", Bool),
                    ("open", Bool),
                ],
                &[POWERED],
                opaque = false
            )
        };
    }
    macro_rules! register_wall {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("east", "none"),
                    ("north", "none"),
                    ("south", "none"),
                    ("west", "none"),
                    ("up", "true"),
                    ("waterlogged", "false"),
                ],
                &[
                    ("east", Enum(&["none", "low", "tall"])),
                    ("north", Enum(&["none", "low", "tall"])),
                    ("south", Enum(&["none", "low", "tall"])),
                    ("up", Bool),
                    WATERLOGGED,
                    ("west", Enum(&["none", "low", "tall"])),
                ],
                opaque = false
            )
        };
    }
    macro_rules! register_door {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("facing", "north"),
                    ("half", "lower"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
                replace: &[
                    FACING_NSWE,
                    ("half", Enum(&["upper", "lower"])),
                    ("hinge", Enum(&["left", "right"])),
                    ("open", Bool),
                ],
                &[POWERED],
                opaque = false
            )
        };
    }
    macro_rules! register_trapdoor {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("open", "false"),
                    ("powered", "false"),
                    ("waterlogged", "false"),
                ],
                replace: &[
                    FACING_NSWE,
                    ("half", Enum(&["top", "bottom"])),
                    ("open", Bool),
                ],
                &[POWERED, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_chest {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("type", "single"),
                    ("facing", "north"),
                    ("waterlogged", "false")
                ],
                &[FACING_NSWE, CHEST_TYPE, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_sign {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("rotation", "0"), ("waterlogged", "false")],
                &[ROTATION_0_15, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_wall_sign {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("facing", "north"), ("waterlogged", "false")],
                &[FACING_NSWE, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_hanging_sign {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[
                    ("attached", "false"),
                    ("rotation", "0"),
                    ("waterlogged", "false"),
                ],
                &[("attached", Bool), ROTATION_0_15, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_wall_hanging_sign {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("facing", "north"), ("waterlogged", "false")],
                &[FACING_NSWE, WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_stained_glass {
        ($identifier:expr) => {
            register!($identifier, opaque = false)
        };
    }
    macro_rules! register_grate {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("waterlogged", "false")],
                &[WATERLOGGED],
                opaque = false
            )
        };
    }
    macro_rules! register_pressure_plate {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("powered", "false")],
                replace: &[POWERED],
                opaque = false
            )
        };
    }
    macro_rules! register_button {
        ($identifier:expr) => {
            register!(
                $identifier,
                override_default: &[("face", "wall"), ("facing", "north"), ("powered", "false")],
                replace: &[
                    ("face", Enum(&["floor", "wall", "ceiling"])),
                    FACING_NSWE,
                    POWERED,
                ],
                opaque = false
            )
        };
    }

    register!("air", opaque = false)?;
    register!("stone")?;
    register!("granite")?;
    register!("polished_granite")?;
    register!("diorite")?;
    register!("polished_diorite")?;
    register!("andesite")?;
    register!("polished_andesite")?;
    register!("grass_block", override_default: &[("snowy", "false")], replace: &[("snowy", Bool)])?;
    register!("dirt")?;
    register!("coarse_dirt")?;
    register!("podzol", override_default: &[("snowy", "false")], replace: &[("snowy", Bool)])?;
    register!("cobblestone")?;
    register!("oak_planks")?;
    register!("spruce_planks")?;
    register!("birch_planks")?;
    register!("jungle_planks")?;
    register!("acacia_planks")?;
    register!("cherry_planks")?;
    register!("dark_oak_planks")?;
    register!("mangrove_planks")?;
    register!("bamboo_planks")?;
    register!("bamboo_mosaic")?;
    register!("oak_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("spruce_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("birch_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("jungle_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("acacia_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("cherry_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!("dark_oak_sapling", &[("stage", Int(0..=1))], opaque = false)?;
    register!(
        "mangrove_propagule",
        override_default: &[
            ("age", "0"),
            ("hanging", "false"),
            ("stage", "0"),
            ("waterlogged", "false"),
        ],
        replace: &[("age", Int(0..=4)), ("hanging", Bool)],
        &[("stage", Int(0..=1)), WATERLOGGED],
        opaque = false
    )?;
    register!("bedrock")?;
    register_liquid!("water")?;
    register_liquid!("lava")?;
    register!("sand")?;
    register!("suspicious_sand")?;
    register!("red_sand")?;
    register!("gravel")?;
    register!("suspicious_gravel")?;
    register!("gold_ore")?;
    register!("deepslate_gold_ore")?;
    register!("iron_ore")?;
    register!("deepslate_iron_ore")?;
    register!("coal_ore")?;
    register!("deepslate_coal_ore")?;
    register!("nether_gold_ore")?;
    {
        macro_rules! register_log {
            ($identifier:expr) => {
                register!($identifier, override_default: &[("axis", "y")])
            };
        }
        register_log!("oak_log")?;
        register_log!("spruce_log")?;
        register_log!("birch_log")?;
        register_log!("jungle_log")?;
        register_log!("acacia_log")?;
        register_log!("cherry_log")?;
        register_log!("dark_oak_log")?;
        register_log!("mangrove_log")?;
        register!("mangrove_roots", override_default: &[("waterlogged", "false")], &[WATERLOGGED])?;
        register_log!("muddy_mangrove_roots")?;
        register_log!("bamboo_block")?;
        register_log!("stripped_spruce_log")?;
        register_log!("stripped_birch_log")?;
        register_log!("stripped_jungle_log")?;
        register_log!("stripped_acacia_log")?;
        register_log!("stripped_cherry_log")?;
        register_log!("stripped_dark_oak_log")?;
        register_log!("stripped_oak_log")?;
        register_log!("stripped_mangrove_log")?;
        register_log!("stripped_bamboo_block")?;
    }
    {
        macro_rules! register_wood {
            ($identifier:expr) => {
                register!($identifier, override_default: &[("axis", "y")])
            };
        }
        register_wood!("oak_wood")?;
        register_wood!("spruce_wood")?;
        register_wood!("birch_wood")?;
        register_wood!("jungle_wood")?;
        register_wood!("acacia_wood")?;
        register_wood!("cherry_wood")?;
        register_wood!("dark_oak_wood")?;
        register_wood!("mangrove_wood")?;
        register_wood!("stripped_oak_wood")?;
        register_wood!("stripped_spruce_wood")?;
        register_wood!("stripped_birch_wood")?;
        register_wood!("stripped_jungle_wood")?;
        register_wood!("stripped_acacia_wood")?;
        register_wood!("stripped_cherry_wood")?;
        register_wood!("stripped_dark_oak_wood")?;
        register_wood!("stripped_mangrove_wood")?;
    }
    {
        macro_rules! register_leaves {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[
                        ("distance", "7"),
                        ("persistent", "false"),
                        ("waterlogged", "false"),
                    ],
                    &[("distance", Int(1..=7)), ("persistent", Bool), WATERLOGGED],
                    opaque = false
                )
            };
        }
        register_leaves!("oak_leaves")?;
        register_leaves!("spruce_leaves")?;
        register_leaves!("birch_leaves")?;
        register_leaves!("jungle_leaves")?;
        register_leaves!("acacia_leaves")?;
        register_leaves!("cherry_leaves")?;
        register_leaves!("dark_oak_leaves")?;
        register_leaves!("mangrove_leaves")?;
        register_leaves!("azalea_leaves")?;
        register_leaves!("flowering_azalea_leaves")?;
    }
    register!("sponge")?;
    register!("wet_sponge")?;
    register!("glass", opaque = false)?;
    register!("lapis_ore")?;
    register!("deepslate_lapis_ore")?;
    register!("lapis_block")?;
    register!(
        "dispenser",
        override_default: &[("facing", "north"), ("triggered", "false")],
        replace: &[FACING_NESWUD],
        &[("triggered", Bool)]
    )?;
    register!("sandstone")?;
    register!("chiseled_sandstone")?;
    register!("cut_sandstone")?;
    register!(
        "note_block",
        override_default: &[("instrument", "harp"), ("note", "0"), ("powered", "false")],
        &[
            (
                "instrument",
                Enum(&[
                    "harp",
                    "basedrum",
                    "snare",
                    "hat",
                    "bass",
                    "flute",
                    "bell",
                    "guitar",
                    "chime",
                    "xylophone",
                    "iron_xylophone",
                    "cow_bell",
                    "didgeridoo",
                    "bit",
                    "banjo",
                    "pling",
                    "zombie",
                    "skeleton",
                    "creeper",
                    "dragon",
                    "wither_skeleton",
                    "piglin",
                    "custom_head",
                ])
            ),
            ("note", Int(0..=24)),
            POWERED,
        ]
    )?;
    {
        macro_rules! register_bed {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[
                        ("facing", "north"),
                        ("occupied", "false"),
                        ("part", "foot"),
                    ],
                    opaque = false
                )
            };
        }
        register_bed!("white_bed")?;
        register_bed!("orange_bed")?;
        register_bed!("magenta_bed")?;
        register_bed!("light_blue_bed")?;
        register_bed!("yellow_bed")?;
        register_bed!("lime_bed")?;
        register_bed!("pink_bed")?;
        register_bed!("gray_bed")?;
        register_bed!("light_gray_bed")?;
        register_bed!("cyan_bed")?;
        register_bed!("purple_bed")?;
        register_bed!("blue_bed")?;
        register_bed!("brown_bed")?;
        register_bed!("green_bed")?;
        register_bed!("red_bed")?;
        register_bed!("black_bed")?;
    }
    register!(
        "powered_rail",
        override_default: &[
            ("powered", "false"),
            ("shape", "north_south"),
            ("waterlogged", "false"),
        ],
        replace: &[
            POWERED,
            ("shape", Enum(&[
                "north_south",
                "east_west",
                "ascending_east",
                "ascending_west",
                "ascending_north",
                "ascending_south",
            ])),
        ],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "detector_rail",
        override_default: &[
            ("powered", "false"),
            ("shape", "north_south"),
            ("waterlogged", "false"),
        ],
        replace: &[
            POWERED,
            ("shape", Enum(&[
                "north_south",
                "east_west",
                "ascending_east",
                "ascending_west",
                "ascending_north",
                "ascending_south",
            ])),
        ],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "sticky_piston",
        override_default: &[("extended", "false"), ("facing", "north")],
        replace: &[("extended", Bool), FACING_NESWUD],
        opaque = false
    )?;
    register!("cobweb", opaque = false)?;
    register!("short_grass", opaque = false)?;
    register!("fern", opaque = false)?;
    register!("dead_bush", opaque = false)?;
    register!("seagrass", opaque = false)?;
    register!(
        "tall_seagrass",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "piston",
        override_default: &[("extended", "false"), ("facing", "north")],
        replace: &[("extended", Bool), FACING_NESWUD],
        opaque = false
    )?;
    register!(
        "piston_head",
        override_default: &[("type", "normal"), ("facing", "north"), ("short", "false")],
        replace: &[FACING_NESWUD, ("short", Bool), ("type", Enum(&["normal", "sticky"]))],
        opaque = false
    )?;
    register!("white_wool")?;
    register!("orange_wool")?;
    register!("magenta_wool")?;
    register!("light_blue_wool")?;
    register!("yellow_wool")?;
    register!("lime_wool")?;
    register!("pink_wool")?;
    register!("gray_wool")?;
    register!("light_gray_wool")?;
    register!("cyan_wool")?;
    register!("purple_wool")?;
    register!("blue_wool")?;
    register!("brown_wool")?;
    register!("green_wool")?;
    register!("red_wool")?;
    register!("black_wool")?;
    register!(
        "moving_piston",
        &[FACING_NESWUD, ("type", Enum(&["normal", "sticky"]))],
        opaque = false
    )?;
    register!("dandelion", opaque = false)?;
    register!("torchflower", opaque = false)?;
    register!("poppy", opaque = false)?;
    register!("blue_orchid", opaque = false)?;
    register!("allium", opaque = false)?;
    register!("azure_bluet", opaque = false)?;
    register!("red_tulip", opaque = false)?;
    register!("orange_tulip", opaque = false)?;
    register!("white_tulip", opaque = false)?;
    register!("pink_tulip", opaque = false)?;
    register!("oxeye_daisy", opaque = false)?;
    register!("cornflower", opaque = false)?;
    register!("wither_rose", opaque = false)?;
    register!("lily_of_the_valley", opaque = false)?;
    register!("brown_mushroom", opaque = false)?;
    register!("red_mushroom", opaque = false)?;
    register!("gold_block")?;
    register!("iron_block")?;
    register!("bricks")?;
    register!("tnt", override_default: &[("unstable", "false")], &[("unstable", Bool)])?;
    register!("bookshelf")?;
    register!(
        "chiseled_bookshelf",
        override_default: &[
            ("facing", "north"),
            ("slot_0_occupied", "false"),
            ("slot_1_occupied", "false"),
            ("slot_2_occupied", "false"),
            ("slot_3_occupied", "false"),
            ("slot_4_occupied", "false"),
            ("slot_5_occupied", "false"),
        ],
        &[
            FACING_NSWE,
            ("slot_0_occupied", Bool),
            ("slot_1_occupied", Bool),
            ("slot_2_occupied", Bool),
            ("slot_3_occupied", Bool),
            ("slot_4_occupied", Bool),
            ("slot_5_occupied", Bool),
        ]
    )?;
    register!("mossy_cobblestone")?;
    register!("obsidian")?;
    register!("torch", opaque = false)?;
    register!("wall_torch", replace: &[FACING_NSWE], opaque = false)?;
    register!(
        "fire",
        override_default: &[
            ("age", "0"),
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("up", "false"),
            ("west", "false"),
        ],
        &[
            AGE_0_15,
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!("soul_fire", &[], opaque = false)?;
    register!("spawner", opaque = false)?;
    register_stairs!("oak_stairs")?;
    register_chest!("chest")?;
    register!(
        "redstone_wire",
        override_default: &[
            ("east", "none"),
            ("north", "none"),
            ("south", "none"),
            ("west", "none"),
            ("power", "0"),
        ],
        &[
            ("east", Enum(&["up", "side", "none"])),
            ("north", Enum(&["up", "side", "none"])),
            ("power", Int(0..=15)),
            ("south", Enum(&["up", "side", "none"])),
            ("west", Enum(&["up", "side", "none"])),
        ],
        opaque = false
    )?;
    register!("diamond_ore")?;
    register!("deepslate_diamond_ore")?;
    register!("diamond_block")?;
    register!("crafting_table")?;
    register!("wheat", opaque = false)?;
    register!("farmland", opaque = false)?;
    register!(
        "furnace",
        override_default: &[("facing", "north"), ("lit", "false")],
        replace: &[FACING_NSWE, LIT]
    )?;
    register_sign!("oak_sign")?;
    register_sign!("spruce_sign")?;
    register_sign!("birch_sign")?;
    register_sign!("acacia_sign")?;
    register_sign!("cherry_sign")?;
    register_sign!("jungle_sign")?;
    register_sign!("dark_oak_sign")?;
    register_sign!("mangrove_sign")?;
    register_sign!("bamboo_sign")?;
    register_door!("oak_door")?;
    register!(
        "ladder",
        override_default: &[("facing", "north"), ("waterlogged", "false")],
        replace: &[FACING_NSWE],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "rail",
        override_default: &[("shape", "north_south"), ("waterlogged", "false")],
        replace: &[("shape", Enum(&[
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
        ]))],
        &[WATERLOGGED],
        opaque = false
    )?;
    register_stairs!("cobblestone_stairs")?;
    register_wall_sign!("oak_wall_sign")?;
    register_wall_sign!("spruce_wall_sign")?;
    register_wall_sign!("birch_wall_sign")?;
    register_wall_sign!("acacia_wall_sign")?;
    register_wall_sign!("cherry_wall_sign")?;
    register_wall_sign!("jungle_wall_sign")?;
    register_wall_sign!("dark_oak_wall_sign")?;
    register_wall_sign!("mangrove_wall_sign")?;
    register_wall_sign!("bamboo_wall_sign")?;
    register_hanging_sign!("oak_hanging_sign")?;
    register_hanging_sign!("spruce_hanging_sign")?;
    register_hanging_sign!("birch_hanging_sign")?;
    register_hanging_sign!("acacia_hanging_sign")?;
    register_hanging_sign!("cherry_hanging_sign")?;
    register_hanging_sign!("jungle_hanging_sign")?;
    register_hanging_sign!("dark_oak_hanging_sign")?;
    register_hanging_sign!("crimson_hanging_sign")?;
    register_hanging_sign!("warped_hanging_sign")?;
    register_hanging_sign!("mangrove_hanging_sign")?;
    register_hanging_sign!("bamboo_hanging_sign")?;
    register_wall_hanging_sign!("oak_wall_hanging_sign")?;
    register_wall_hanging_sign!("spruce_wall_hanging_sign")?;
    register_wall_hanging_sign!("birch_wall_hanging_sign")?;
    register_wall_hanging_sign!("acacia_wall_hanging_sign")?;
    register_wall_hanging_sign!("cherry_wall_hanging_sign")?;
    register_wall_hanging_sign!("jungle_wall_hanging_sign")?;
    register_wall_hanging_sign!("dark_oak_wall_hanging_sign")?;
    register_wall_hanging_sign!("mangrove_wall_hanging_sign")?;
    register_wall_hanging_sign!("crimson_wall_hanging_sign")?;
    register_wall_hanging_sign!("warped_wall_hanging_sign")?;
    register_wall_hanging_sign!("bamboo_wall_hanging_sign")?;
    register!(
        "lever",
        override_default: &[("face", "wall"), ("facing", "north"), ("powered", "false")],
        replace: &[("face", Enum(&["floor", "wall", "ceiling"])), FACING_NSWE, POWERED],
        opaque = false
    )?;
    register_pressure_plate!("stone_pressure_plate")?;
    register!(
        "iron_door",
        override_default: &[
            ("facing", "north"),
            ("half", "lower"),
            ("hinge", "left"),
            ("open", "false"),
            ("powered", "false"),
        ],
        replace: &[
            FACING_NSWE,
            ("half", Enum(&["upper", "lower"])),
            ("hinge", Enum(&["left", "right"])),
            ("open", Bool),
        ],
        &[POWERED],
        opaque = false
    )?;
    register_pressure_plate!("oak_pressure_plate")?;
    register_pressure_plate!("spruce_pressure_plate")?;
    register_pressure_plate!("birch_pressure_plate")?;
    register_pressure_plate!("jungle_pressure_plate")?;
    register_pressure_plate!("acacia_pressure_plate")?;
    register_pressure_plate!("cherry_pressure_plate")?;
    register_pressure_plate!("dark_oak_pressure_plate")?;
    register_pressure_plate!("mangrove_pressure_plate")?;
    register_pressure_plate!("bamboo_pressure_plate")?;
    register!("redstone_ore", override_default: &[("lit", "false")], &[LIT])?;
    register!("deepslate_redstone_ore", override_default: &[("lit", "false")], &[LIT])?;
    register!("redstone_torch", replace: &[LIT], opaque = false)?;
    register!("redstone_wall_torch", replace: &[FACING_NSWE, LIT], opaque = false)?;
    register_button!("stone_button")?;
    register!("snow", opaque = false)?;
    register!("ice", opaque = false)?;
    register!("snow_block")?;
    register!("cactus", &[AGE_0_15], opaque = false)?;
    register!("clay")?;
    register!("sugar_cane", &[AGE_0_15], opaque = false)?;
    register!("jukebox", override_default: &[("has_record", "false")], &[("has_record", Bool)])?;
    register_fence!("oak_fence")?;
    register!("netherrack")?;
    register!("soul_sand", opaque = false)?;
    register!("soul_soil")?;
    register!("basalt", override_default: &[("axis", "y")])?;
    register!("polished_basalt", override_default: &[("axis", "y")])?;
    register!("soul_torch", opaque = false)?;
    register!("soul_wall_torch", replace: &[FACING_NSWE], opaque = false)?;
    register!("glowstone")?;
    register!("nether_portal")?;
    register!("carved_pumpkin", replace: &[FACING_NSWE])?;
    register!("jack_o_lantern", replace: &[FACING_NSWE])?;
    register!("cake", opaque = false)?;
    register!(
        "repeater",
        override_default: &[
            ("delay", "1"),
            ("facing", "north"),
            ("locked", "false"),
            ("powered", "false"),
        ],
        replace: &[("delay", Int(1..=4)), FACING_NSWE, ("locked", Bool), ("powered", Bool)],
        opaque = false
    )?;
    register_stained_glass!("white_stained_glass")?;
    register_stained_glass!("orange_stained_glass")?;
    register_stained_glass!("magenta_stained_glass")?;
    register_stained_glass!("light_blue_stained_glass")?;
    register_stained_glass!("yellow_stained_glass")?;
    register_stained_glass!("lime_stained_glass")?;
    register_stained_glass!("pink_stained_glass")?;
    register_stained_glass!("gray_stained_glass")?;
    register_stained_glass!("light_gray_stained_glass")?;
    register_stained_glass!("cyan_stained_glass")?;
    register_stained_glass!("purple_stained_glass")?;
    register_stained_glass!("blue_stained_glass")?;
    register_stained_glass!("brown_stained_glass")?;
    register_stained_glass!("green_stained_glass")?;
    register_stained_glass!("red_stained_glass")?;
    register_stained_glass!("black_stained_glass")?;
    register_trapdoor!("oak_trapdoor")?;
    register_trapdoor!("spruce_trapdoor")?;
    register_trapdoor!("birch_trapdoor")?;
    register_trapdoor!("jungle_trapdoor")?;
    register_trapdoor!("acacia_trapdoor")?;
    register_trapdoor!("cherry_trapdoor")?;
    register_trapdoor!("dark_oak_trapdoor")?;
    register_trapdoor!("mangrove_trapdoor")?;
    register_trapdoor!("bamboo_trapdoor")?;
    register!("stone_bricks")?;
    register!("mossy_stone_bricks")?;
    register!("cracked_stone_bricks")?;
    register!("chiseled_stone_bricks")?;
    register!("packed_mud")?;
    register!("mud_bricks")?;
    register!("infested_stone")?;
    register!("infested_cobblestone")?;
    register!("infested_stone_bricks")?;
    register!("infested_mossy_stone_bricks")?;
    register!("infested_cracked_stone_bricks")?;
    register!("infested_chiseled_stone_bricks")?;
    {
        macro_rules! register_mushroom_block {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    &[
                        ("down", Bool),
                        ("east", Bool),
                        ("north", Bool),
                        ("south", Bool),
                        ("up", Bool),
                        ("west", Bool),
                    ]
                )
            };
        }
        register_mushroom_block!("brown_mushroom_block")?;
        register_mushroom_block!("red_mushroom_block")?;
        register_mushroom_block!("mushroom_stem")?;
    }
    register!(
        "iron_bars",
        override_default: &[
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("west", "false"),
            ("waterlogged", "false"),
        ],
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            WATERLOGGED,
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!(
        "chain",
        override_default: &[("axis", "y"), ("waterlogged", "false")],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "glass_pane",
        override_default: &[
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("west", "false"),
            ("waterlogged", "false"),
        ],
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            WATERLOGGED,
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!("pumpkin")?;
    register!("melon")?;
    register!("attached_pumpkin_stem", replace: &[FACING_NSWE], opaque = false)?;
    register!("attached_melon_stem", replace: &[FACING_NSWE], opaque = false)?;
    register!("pumpkin_stem", opaque = false)?;
    register!("melon_stem", opaque = false)?;
    register!(
        "vine",
        override_default: &[
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("up", "false"),
            ("west", "false"),
        ],
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!(
        "glow_lichen",
        override_default: &[
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("west", "false"),
            ("up", "false"),
            ("down", "false"),
            ("waterlogged", "false"),
        ],
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            WATERLOGGED,
            ("west", Bool),
        ],
        opaque = false
    )?;
    register_fence_gate!("oak_fence_gate")?;
    register_stairs!("brick_stairs")?;
    register_stairs!("stone_brick_stairs")?;
    register_stairs!("mud_brick_stairs")?;
    register!("mycelium", override_default: &[("snowy", "false")], replace: &[("snowy", Bool)])?;
    register!("lily_pad", opaque = false)?;
    register!("nether_bricks")?;
    register_fence!("nether_brick_fence")?;
    register_stairs!("nether_brick_stairs")?;
    register!("nether_wart", opaque = false)?;
    register!("enchanting_table", opaque = false)?;
    register!(
        "brewing_stand",
        override_default: &[
            ("has_bottle_0", "false"),
            ("has_bottle_1", "false"),
            ("has_bottle_2", "false"),
        ],
        &[
            ("has_bottle_0", Bool),
            ("has_bottle_1", Bool),
            ("has_bottle_2", Bool),
        ],
        opaque = false
    )?;
    register!("cauldron", opaque = false)?;
    register!("water_cauldron", opaque = false)?;
    register!("lava_cauldron", opaque = false)?;
    register!("powder_snow_cauldron", opaque = false)?;
    register!("end_portal", opaque = false)?;
    register!(
        "end_portal_frame",
        override_default: &[("eye", "false"), ("facing", "north")],
        replace: &[("eye", Bool), FACING_NSWE],
        opaque = false
    )?;
    register!("end_stone")?;
    register!("dragon_egg", opaque = false)?;
    register!("redstone_lamp", override_default: &[("lit", "false")], replace: &[LIT])?;
    register!("cocoa", replace: &[("age", Int(0..=2)), FACING_NSWE], opaque = false)?;
    register_stairs!("sandstone_stairs")?;
    register!("emerald_ore")?;
    register!("deepslate_emerald_ore")?;
    register!(
        "ender_chest",
        override_default: &[("facing", "north"), ("waterlogged", "false")],
        &[FACING_NSWE, WATERLOGGED],
        opaque = false
    )?;
    register!(
        "tripwire_hook",
        override_default: &[
            ("attached", "false"),
            ("facing", "north"),
            ("powered", "false"),
        ],
        replace: &[("attached", Bool), FACING_NSWE, POWERED],
        opaque = false
    )?;
    register!(
        "tripwire",
        override_default: &[
            ("attached", "false"),
            ("disarmed", "false"),
            ("east", "false"),
            ("north", "false"),
            ("powered", "false"),
            ("south", "false"),
            ("west", "false"),
        ],
        full_custom: &[
            ("attached", Bool),
            ("disarmed", Bool),
            ("east", Bool),
            ("north", Bool),
            POWERED,
            ("south", Bool),
            ("west", Bool),
        ],
        skips: &["disarmed", "powered"],
        opaque = false
    )?;
    register!("emerald_block")?;
    register_stairs!("spruce_stairs")?;
    register_stairs!("birch_stairs")?;
    register_stairs!("jungle_stairs")?;
    register!(
        "command_block",
        override_default: &[("conditional", "false"), ("facing", "north")],
        replace: &[("conditional", Bool), FACING_NESWUD]
    )?;
    register!("beacon", opaque = false)?;
    register_wall!("cobblestone_wall")?;
    register_wall!("mossy_cobblestone_wall")?;
    register!("flower_pot", opaque = false)?;
    register!("potted_torchflower", opaque = false)?;
    register!("potted_oak_sapling", opaque = false)?;
    register!("potted_spruce_sapling", opaque = false)?;
    register!("potted_birch_sapling", opaque = false)?;
    register!("potted_jungle_sapling", opaque = false)?;
    register!("potted_acacia_sapling", opaque = false)?;
    register!("potted_cherry_sapling", opaque = false)?;
    register!("potted_dark_oak_sapling", opaque = false)?;
    register!("potted_mangrove_propagule", opaque = false)?;
    register!("potted_fern", opaque = false)?;
    register!("potted_dandelion", opaque = false)?;
    register!("potted_poppy", opaque = false)?;
    register!("potted_blue_orchid", opaque = false)?;
    register!("potted_allium", opaque = false)?;
    register!("potted_azure_bluet", opaque = false)?;
    register!("potted_red_tulip", opaque = false)?;
    register!("potted_orange_tulip", opaque = false)?;
    register!("potted_white_tulip", opaque = false)?;
    register!("potted_pink_tulip", opaque = false)?;
    register!("potted_oxeye_daisy", opaque = false)?;
    register!("potted_cornflower", opaque = false)?;
    register!("potted_lily_of_the_valley", opaque = false)?;
    register!("potted_wither_rose", opaque = false)?;
    register!("potted_red_mushroom", opaque = false)?;
    register!("potted_brown_mushroom", opaque = false)?;
    register!("potted_dead_bush", opaque = false)?;
    register!("potted_cactus", opaque = false)?;
    register!("carrots", opaque = false)?;
    register!("potatoes", opaque = false)?;
    register_button!("oak_button")?;
    register_button!("spruce_button")?;
    register_button!("birch_button")?;
    register_button!("jungle_button")?;
    register_button!("acacia_button")?;
    register_button!("cherry_button")?;
    register_button!("dark_oak_button")?;
    register_button!("mangrove_button")?;
    register_button!("bamboo_button")?;
    {
        macro_rules! register_head {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("powered", "false"), ("rotation", "0")],
                    &[POWERED, ROTATION_0_15],
                    opaque = false
                )
            };
        }
        macro_rules! register_wall_head {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("facing", "north"), ("powered", "false")],
                    &[FACING_NSWE, POWERED],
                    opaque = false
                )
            };
        }
        register_head!("skeleton_skull")?;
        register_wall_head!("skeleton_wall_skull")?;
        register_head!("wither_skeleton_skull")?;
        register_wall_head!("wither_skeleton_wall_skull")?;
        register_head!("zombie_head")?;
        register_wall_head!("zombie_wall_head")?;
        register_head!("player_head")?;
        register_wall_head!("player_wall_head")?;
        register_head!("creeper_head")?;
        register_wall_head!("creeper_wall_head")?;
        register_head!("dragon_head")?;
        register_wall_head!("dragon_wall_head")?;
        register_head!("piglin_head")?;
        register_wall_head!("piglin_wall_head")?;
    }
    register!("anvil", replace: &[FACING_NSWE], opaque = false)?;
    register!("chipped_anvil", replace: &[FACING_NSWE], opaque = false)?;
    register!("damaged_anvil", replace: &[FACING_NSWE], opaque = false)?;
    register_chest!("trapped_chest")?;
    {
        macro_rules! register_weighted_pressure_plate {
            ($identifier:expr) => {
                register!($identifier, replace: &[("power", Int(0..=15))], opaque = false)
            };
        }
        register_weighted_pressure_plate!("light_weighted_pressure_plate")?;
        register_weighted_pressure_plate!("heavy_weighted_pressure_plate")?;
    }
    register!(
        "comparator",
        override_default: &[("facing", "north"), ("mode", "compare"), ("powered", "false")],
        replace: &[FACING_NSWE, ("mode", Enum(&["compare", "subtract"])), POWERED],
        opaque = false
    )?;
    register!(
        "daylight_detector",
        override_default: &[("inverted", "false"), ("power", "0")],
        replace: &[("inverted", Bool)],
        &[("power", Int(0..=15))],
        opaque = false
    )?;
    register!("redstone_block")?;
    register!("nether_quartz_ore")?;
    register!(
        "hopper",
        full_custom: &[
            ("enabled", Bool),
            ("facing", Enum(&["down", "north", "south", "west", "east"])),
        ],
        skips: &["enabled"],
        opaque = false
    )?;
    register!("quartz_block")?;
    register!("chiseled_quartz_block")?;
    register!("quartz_pillar", override_default: &[("axis", "y")], opaque = false)?;
    register!(
        "quartz_stairs",
        override_default: &[
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ],
        replace: &[
            FACING_NSWE,
            ("half", Enum(&["top", "bottom"])),
            ("shape", Enum(&[
                "straight",
                "inner_left",
                "inner_right",
                "outer_left",
                "outer_right",
            ])),
        ],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "activator_rail",
        override_default: &[
            ("powered", "false"),
            ("shape", "north_south"),
            ("waterlogged", "false")
        ],
        replace: &[
            POWERED,
            ("shape", Enum(&[
                "north_south",
                "east_west",
                "ascending_east",
                "ascending_west",
                "ascending_north",
                "ascending_south",
            ]))
        ],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "dropper",
        override_default: &[("facing", "north"), ("triggered", "false")],
        replace: &[FACING_NESWUD],
        &[("triggered", Bool)]
    )?;
    register!("white_terracotta")?;
    register!("orange_terracotta")?;
    register!("magenta_terracotta")?;
    register!("light_blue_terracotta")?;
    register!("yellow_terracotta")?;
    register!("lime_terracotta")?;
    register!("pink_terracotta")?;
    register!("gray_terracotta")?;
    register!("light_gray_terracotta")?;
    register!("cyan_terracotta")?;
    register!("purple_terracotta")?;
    register!("blue_terracotta")?;
    register!("brown_terracotta")?;
    register!("green_terracotta")?;
    register!("red_terracotta")?;
    register!("black_terracotta")?;
    {
        macro_rules! register_glass_pane {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[
                        ("east", "false"),
                        ("north", "false"),
                        ("south", "false"),
                        ("west", "false"),
                        ("waterlogged", "false"),
                    ],
                    &[
                        ("east", Bool),
                        ("north", Bool),
                        ("south", Bool),
                        WATERLOGGED,
                        ("west", Bool),
                    ],
                    opaque = false
                )
            };
        }
        register_glass_pane!("white_stained_glass_pane")?;
        register_glass_pane!("orange_stained_glass_pane")?;
        register_glass_pane!("magenta_stained_glass_pane")?;
        register_glass_pane!("light_blue_stained_glass_pane")?;
        register_glass_pane!("yellow_stained_glass_pane")?;
        register_glass_pane!("lime_stained_glass_pane")?;
        register_glass_pane!("pink_stained_glass_pane")?;
        register_glass_pane!("gray_stained_glass_pane")?;
        register_glass_pane!("light_gray_stained_glass_pane")?;
        register_glass_pane!("cyan_stained_glass_pane")?;
        register_glass_pane!("purple_stained_glass_pane")?;
        register_glass_pane!("blue_stained_glass_pane")?;
        register_glass_pane!("brown_stained_glass_pane")?;
        register_glass_pane!("green_stained_glass_pane")?;
        register_glass_pane!("red_stained_glass_pane")?;
        register_glass_pane!("black_stained_glass_pane")?;
    }
    register_stairs!("acacia_stairs")?;
    register_stairs!("cherry_stairs")?;
    register_stairs!("dark_oak_stairs")?;
    register_stairs!("mangrove_stairs")?;
    register_stairs!("bamboo_stairs")?;
    register_stairs!("bamboo_mosaic_stairs")?;
    register!("slime_block", opaque = false)?;
    register!(
        "barrier",
        override_default: &[("waterlogged", "false")],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "light",
        override_default: &[("level", "15"), ("waterlogged", "false")],
        replace: &[("level", Int(0..=15))],
        &[WATERLOGGED],
        opaque = false
    )?;
    register_trapdoor!("iron_trapdoor")?;
    register!("prismarine")?;
    register!("prismarine_bricks")?;
    register!("dark_prismarine")?;
    register_stairs!("prismarine_stairs")?;
    register_stairs!("prismarine_brick_stairs")?;
    register_stairs!("dark_prismarine_stairs")?;
    register_slab!("prismarine_slab")?;
    register_slab!("prismarine_brick_slab")?;
    register_slab!("dark_prismarine_slab")?;
    register!("sea_lantern", opaque = false)?;
    register!("hay_block", override_default: &[("axis", "y")])?;
    register!("white_carpet", opaque = false)?;
    register!("orange_carpet", opaque = false)?;
    register!("magenta_carpet", opaque = false)?;
    register!("light_blue_carpet", opaque = false)?;
    register!("yellow_carpet", opaque = false)?;
    register!("lime_carpet", opaque = false)?;
    register!("pink_carpet", opaque = false)?;
    register!("gray_carpet", opaque = false)?;
    register!("light_gray_carpet", opaque = false)?;
    register!("cyan_carpet", opaque = false)?;
    register!("purple_carpet", opaque = false)?;
    register!("blue_carpet", opaque = false)?;
    register!("brown_carpet", opaque = false)?;
    register!("green_carpet", opaque = false)?;
    register!("red_carpet", opaque = false)?;
    register!("black_carpet", opaque = false)?;
    register!("terracotta")?;
    register!("coal_block")?;
    register!("packed_ice")?;
    register!(
        "sunflower",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "lilac",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "rose_bush",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "peony",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "tall_grass",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "large_fern",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    {
        macro_rules! register_banner {
            ($identifier:expr) => {
                register!($identifier, &[ROTATION_0_15], opaque = false)
            };
        }
        macro_rules! register_wall_banner {
            ($identifier:expr) => {
                register!($identifier, &[FACING_NSWE], opaque = false)
            };
        }
        register_banner!("white_banner")?;
        register_banner!("orange_banner")?;
        register_banner!("magenta_banner")?;
        register_banner!("light_blue_banner")?;
        register_banner!("yellow_banner")?;
        register_banner!("lime_banner")?;
        register_banner!("pink_banner")?;
        register_banner!("gray_banner")?;
        register_banner!("light_gray_banner")?;
        register_banner!("cyan_banner")?;
        register_banner!("purple_banner")?;
        register_banner!("blue_banner")?;
        register_banner!("brown_banner")?;
        register_banner!("green_banner")?;
        register_banner!("red_banner")?;
        register_banner!("black_banner")?;
        register_wall_banner!("white_wall_banner")?;
        register_wall_banner!("orange_wall_banner")?;
        register_wall_banner!("magenta_wall_banner")?;
        register_wall_banner!("light_blue_wall_banner")?;
        register_wall_banner!("yellow_wall_banner")?;
        register_wall_banner!("lime_wall_banner")?;
        register_wall_banner!("pink_wall_banner")?;
        register_wall_banner!("gray_wall_banner")?;
        register_wall_banner!("light_gray_wall_banner")?;
        register_wall_banner!("cyan_wall_banner")?;
        register_wall_banner!("purple_wall_banner")?;
        register_wall_banner!("blue_wall_banner")?;
        register_wall_banner!("brown_wall_banner")?;
        register_wall_banner!("green_wall_banner")?;
        register_wall_banner!("red_wall_banner")?;
        register_wall_banner!("black_wall_banner")?;
    }
    register!("red_sandstone")?;
    register!("chiseled_red_sandstone")?;
    register!("cut_red_sandstone")?;
    register_stairs!("red_sandstone_stairs")?;
    register_slab!("oak_slab")?;
    register_slab!("spruce_slab")?;
    register_slab!("birch_slab")?;
    register_slab!("jungle_slab")?;
    register_slab!("acacia_slab")?;
    register_slab!("cherry_slab")?;
    register_slab!("dark_oak_slab")?;
    register_slab!("mangrove_slab")?;
    register_slab!("bamboo_slab")?;
    register_slab!("bamboo_mosaic_slab")?;
    register_slab!("stone_slab")?;
    register_slab!("smooth_stone_slab")?;
    register_slab!("sandstone_slab")?;
    register_slab!("cut_sandstone_slab")?;
    register_slab!("petrified_oak_slab")?;
    register_slab!("cobblestone_slab")?;
    register_slab!("brick_slab")?;
    register_slab!("stone_brick_slab")?;
    register_slab!("mud_brick_slab")?;
    register_slab!("nether_brick_slab")?;
    register_slab!("quartz_slab")?;
    register_slab!("red_sandstone_slab")?;
    register_slab!("cut_red_sandstone_slab")?;
    register_slab!("purpur_slab")?;
    register!("smooth_stone")?;
    register!("smooth_sandstone")?;
    register!("smooth_quartz")?;
    register!("smooth_red_sandstone")?;
    register_fence_gate!("spruce_fence_gate")?;
    register_fence_gate!("birch_fence_gate")?;
    register_fence_gate!("jungle_fence_gate")?;
    register_fence_gate!("acacia_fence_gate")?;
    register_fence_gate!("cherry_fence_gate")?;
    register_fence_gate!("dark_oak_fence_gate")?;
    register_fence_gate!("mangrove_fence_gate")?;
    register_fence_gate!("bamboo_fence_gate")?;
    register_fence!("spruce_fence")?;
    register_fence!("birch_fence")?;
    register_fence!("jungle_fence")?;
    register_fence!("acacia_fence")?;
    register_fence!("cherry_fence")?;
    register_fence!("dark_oak_fence")?;
    register_fence!("mangrove_fence")?;
    register_fence!("bamboo_fence")?;
    register_door!("spruce_door")?;
    register_door!("birch_door")?;
    register_door!("jungle_door")?;
    register_door!("acacia_door")?;
    register_door!("cherry_door")?;
    register_door!("dark_oak_door")?;
    register_door!("mangrove_door")?;
    register_door!("bamboo_door")?;
    register!(
        "end_rod",
        override_default: &[("facing", "up")],
        replace: &[FACING_NESWUD],
        opaque = false
    )?;
    register!(
        "chorus_plant",
        override_default: &[
            ("down", "false"),
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("up", "false"),
            ("west", "false"),
        ],
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!("chorus_flower", opaque = false)?;
    register!("purpur_block")?;
    register!("purpur_pillar", override_default: &[("axis", "y")])?;
    register_stairs!("purpur_stairs")?;
    register!("end_stone_bricks")?;
    register!("torchflower_crop", opaque = false)?;
    register!(
        "pitcher_crop",
        override_default: &[("age", "0"), ("half", "lower")],
        replace: &[("age", Int(0..=4)), ("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!(
        "pitcher_plant",
        override_default: &[("half", "lower")],
        replace: &[("half", Enum(&["upper", "lower"]))],
        opaque = false
    )?;
    register!("beetroots", opaque = false)?;
    register!("dirt_path", opaque = false)?;
    register!("end_gateway")?;
    register!(
        "repeating_command_block",
        override_default: &[("conditional", "false"), ("facing", "north")],
        replace: &[("conditional", Bool), FACING_NESWUD]
    )?;
    register!(
        "chain_command_block",
        override_default: &[("conditional", "false"), ("facing", "north")],
        replace: &[("conditional", Bool), FACING_NESWUD]
    )?;
    register!("frosted_ice")?;
    register!("magma_block")?;
    register!("nether_wart_block")?;
    register!("red_nether_bricks")?;
    register!("bone_block", override_default: &[("axis", "y")])?;
    register!("structure_void", opaque = false)?;
    register!(
        "observer",
        override_default: &[("facing", "south"), ("powered", "false")],
        replace: &[FACING_NESWUD, POWERED]
    )?;
    {
        macro_rules! register_shulker_box {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("facing", "up")],
                    &[FACING_NESWUD],
                    opaque = false
                )
            };
        }
        register_shulker_box!("shulker_box")?;
        register_shulker_box!("white_shulker_box")?;
        register_shulker_box!("orange_shulker_box")?;
        register_shulker_box!("magenta_shulker_box")?;
        register_shulker_box!("light_blue_shulker_box")?;
        register_shulker_box!("yellow_shulker_box")?;
        register_shulker_box!("lime_shulker_box")?;
        register_shulker_box!("pink_shulker_box")?;
        register_shulker_box!("gray_shulker_box")?;
        register_shulker_box!("light_gray_shulker_box")?;
        register_shulker_box!("cyan_shulker_box")?;
        register_shulker_box!("purple_shulker_box")?;
        register_shulker_box!("blue_shulker_box")?;
        register_shulker_box!("brown_shulker_box")?;
        register_shulker_box!("green_shulker_box")?;
        register_shulker_box!("red_shulker_box")?;
        register_shulker_box!("black_shulker_box")?;
    }
    {
        macro_rules! register_glazed_terracotta {
            ($identifier:expr) => {
                register!($identifier, replace: &[FACING_NSWE])
            };
        }
        register_glazed_terracotta!("white_glazed_terracotta")?;
        register_glazed_terracotta!("orange_glazed_terracotta")?;
        register_glazed_terracotta!("magenta_glazed_terracotta")?;
        register_glazed_terracotta!("light_blue_glazed_terracotta")?;
        register_glazed_terracotta!("yellow_glazed_terracotta")?;
        register_glazed_terracotta!("lime_glazed_terracotta")?;
        register_glazed_terracotta!("pink_glazed_terracotta")?;
        register_glazed_terracotta!("gray_glazed_terracotta")?;
        register_glazed_terracotta!("light_gray_glazed_terracotta")?;
        register_glazed_terracotta!("cyan_glazed_terracotta")?;
        register_glazed_terracotta!("purple_glazed_terracotta")?;
        register_glazed_terracotta!("blue_glazed_terracotta")?;
        register_glazed_terracotta!("brown_glazed_terracotta")?;
        register_glazed_terracotta!("green_glazed_terracotta")?;
        register_glazed_terracotta!("red_glazed_terracotta")?;
        register_glazed_terracotta!("black_glazed_terracotta")?;
    }
    register!("white_concrete")?;
    register!("orange_concrete")?;
    register!("magenta_concrete")?;
    register!("light_blue_concrete")?;
    register!("yellow_concrete")?;
    register!("lime_concrete")?;
    register!("pink_concrete")?;
    register!("gray_concrete")?;
    register!("light_gray_concrete")?;
    register!("cyan_concrete")?;
    register!("purple_concrete")?;
    register!("blue_concrete")?;
    register!("brown_concrete")?;
    register!("green_concrete")?;
    register!("red_concrete")?;
    register!("black_concrete")?;
    register!("white_concrete_powder")?;
    register!("orange_concrete_powder")?;
    register!("magenta_concrete_powder")?;
    register!("light_blue_concrete_powder")?;
    register!("yellow_concrete_powder")?;
    register!("lime_concrete_powder")?;
    register!("pink_concrete_powder")?;
    register!("gray_concrete_powder")?;
    register!("light_gray_concrete_powder")?;
    register!("cyan_concrete_powder")?;
    register!("purple_concrete_powder")?;
    register!("blue_concrete_powder")?;
    register!("brown_concrete_powder")?;
    register!("green_concrete_powder")?;
    register!("red_concrete_powder")?;
    register!("black_concrete_powder")?;
    register!("kelp", &[("age", Int(0..=25))], opaque = false)?;
    register!("kelp_plant", opaque = false)?;
    register!("dried_kelp_block")?;
    register!("turtle_egg", opaque = false)?;
    register!("sniffer_egg", opaque = false)?;
    register!("dead_tube_coral_block")?;
    register!("dead_brain_coral_block")?;
    register!("dead_bubble_coral_block")?;
    register!("dead_fire_coral_block")?;
    register!("dead_horn_coral_block")?;
    register!("tube_coral_block")?;
    register!("brain_coral_block")?;
    register!("bubble_coral_block")?;
    register!("fire_coral_block")?;
    register!("horn_coral_block")?;
    {
        macro_rules! register_coral {
            ($identifier:expr) => {
                register!($identifier, &[WATERLOGGED], opaque = false)
            };
            ($identifier:expr, replace: $replacement_variants:expr) => {
                register!(
                    $identifier,
                    replace: $replacement_variants,
                    &[WATERLOGGED],
                    opaque = false
                )
            };
        }
        register_coral!("dead_tube_coral")?;
        register_coral!("dead_brain_coral")?;
        register_coral!("dead_bubble_coral")?;
        register_coral!("dead_fire_coral")?;
        register_coral!("dead_horn_coral")?;
        register_coral!("tube_coral")?;
        register_coral!("brain_coral")?;
        register_coral!("bubble_coral")?;
        register_coral!("fire_coral")?;
        register_coral!("horn_coral")?;
        register_coral!("dead_tube_coral_fan")?;
        register_coral!("dead_brain_coral_fan")?;
        register_coral!("dead_bubble_coral_fan")?;
        register_coral!("dead_fire_coral_fan")?;
        register_coral!("dead_horn_coral_fan")?;
        register_coral!("tube_coral_fan")?;
        register_coral!("brain_coral_fan")?;
        register_coral!("bubble_coral_fan")?;
        register_coral!("fire_coral_fan")?;
        register_coral!("horn_coral_fan")?;
    }
    {
        macro_rules! register_coral_wall_fan {
            ($identifier:expr) => {
                register!($identifier, replace: &[FACING_NSWE], &[WATERLOGGED], opaque = false)
            };
        }
        register_coral_wall_fan!("dead_tube_coral_wall_fan")?;
        register_coral_wall_fan!("dead_brain_coral_wall_fan")?;
        register_coral_wall_fan!("dead_bubble_coral_wall_fan")?;
        register_coral_wall_fan!("dead_fire_coral_wall_fan")?;
        register_coral_wall_fan!("dead_horn_coral_wall_fan")?;
        register_coral_wall_fan!("tube_coral_wall_fan")?;
        register_coral_wall_fan!("brain_coral_wall_fan")?;
        register_coral_wall_fan!("bubble_coral_wall_fan")?;
        register_coral_wall_fan!("fire_coral_wall_fan")?;
        register_coral_wall_fan!("horn_coral_wall_fan")?;
    }
    register!(
        "sea_pickle",
        override_default: &[("pickles", "1"), ("waterlogged", "true")],
        replace: &[("pickles", Int(1..=4)), ("waterlogged", Bool)],
        opaque = false
    )?;
    register!("blue_ice")?;
    register!("conduit", &[WATERLOGGED], opaque = false)?;
    register!("bamboo_sapling", opaque = false)?;
    register!(
        "bamboo",
        &[
            ("age", Int(0..=1)),
            ("leaves", Enum(&["none", "small", "large"])),
            ("stage", Int(0..=1))
        ],
        opaque = false
    )?;
    register!("potted_bamboo", opaque = false)?;
    register!("void_air", opaque = false)?;
    register!("cave_air", opaque = false)?;
    register!("bubble_column", &[("drag", Bool)], opaque = false)?;
    register_stairs!("polished_granite_stairs")?;
    register_stairs!("smooth_red_sandstone_stairs")?;
    register_stairs!("mossy_stone_brick_stairs")?;
    register_stairs!("polished_diorite_stairs")?;
    register_stairs!("mossy_cobblestone_stairs")?;
    register_stairs!("end_stone_brick_stairs")?;
    register_stairs!("stone_stairs")?;
    register_stairs!("smooth_sandstone_stairs")?;
    register_stairs!("smooth_quartz_stairs")?;
    register_stairs!("granite_stairs")?;
    register_stairs!("andesite_stairs")?;
    register_stairs!("red_nether_brick_stairs")?;
    register_stairs!("polished_andesite_stairs")?;
    register_stairs!("diorite_stairs")?;
    register_slab!("polished_granite_slab")?;
    register_slab!("smooth_red_sandstone_slab")?;
    register_slab!("mossy_stone_brick_slab")?;
    register_slab!("polished_diorite_slab")?;
    register_slab!("mossy_cobblestone_slab")?;
    register_slab!("end_stone_brick_slab")?;
    register_slab!("smooth_sandstone_slab")?;
    register_slab!("smooth_quartz_slab")?;
    register_slab!("granite_slab")?;
    register_slab!("andesite_slab")?;
    register_slab!("red_nether_brick_slab")?;
    register_slab!("polished_andesite_slab")?;
    register_slab!("diorite_slab")?;
    register_wall!("brick_wall")?;
    register_wall!("prismarine_wall")?;
    register_wall!("red_sandstone_wall")?;
    register_wall!("mossy_stone_brick_wall")?;
    register_wall!("granite_wall")?;
    register_wall!("stone_brick_wall")?;
    register_wall!("mud_brick_wall")?;
    register_wall!("nether_brick_wall")?;
    register_wall!("andesite_wall")?;
    register_wall!("red_nether_brick_wall")?;
    register_wall!("sandstone_wall")?;
    register_wall!("end_stone_brick_wall")?;
    register_wall!("diorite_wall")?;
    register!(
        "scaffolding",
        override_default: &[("bottom", "false"), ("distance", "7"), ("waterlogged", "false")],
        replace: &[("bottom", Bool)],
        &[("distance", Int(0..=7)), WATERLOGGED],
        opaque = false
    )?;
    register!("loom", replace: &[FACING_NSWE])?;
    register!(
        "barrel",
        override_default: &[("facing", "north"), ("open", "false")],
        replace: &[FACING_NESWUD, ("open", Bool)]
    )?;
    register!(
        "smoker",
        override_default: &[("facing", "north"), ("lit", "false")],
        replace: &[FACING_NSWE, LIT]
    )?;
    register!(
        "blast_furnace",
        override_default: &[("facing", "north"), ("lit", "false")],
        replace: &[FACING_NSWE, LIT]
    )?;
    register!("cartography_table")?;
    register!("fletching_table")?;
    register!(
        "grindstone",
        override_default: &[("face", "wall"), ("facing", "north")],
        replace: &[("face", Enum(&["floor", "wall", "ceiling"])), FACING_NSWE],
        opaque = false
    )?;
    register!(
        "lectern",
        override_default: &[("facing", "north"), ("has_book", "false"), ("powered", "false")],
        replace: &[FACING_NSWE],
        &[("has_book", Bool), POWERED],
        opaque = false
    )?;
    register!("smithing_table")?;
    register!("stonecutter", replace: &[FACING_NSWE], opaque = false)?;
    register!(
        "bell",
        override_default: &[("attachment", "floor"), ("facing", "north"), ("powered", "false")],
        replace: &[
            ("attachment", Enum(&["floor", "ceiling", "single_wall", "double_wall"])),
            FACING_NSWE,
        ],
        &[POWERED],
        opaque = false
    )?;
    {
        macro_rules! register_lantern {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("hanging", "false"), ("waterlogged", "false")],
                    replace: &[("hanging", Bool)],
                    &[WATERLOGGED],
                    opaque = false
                )
            };
        }
        register_lantern!("lantern")?;
        register_lantern!("soul_lantern")?;
    }
    register!(
        "campfire",
        override_default: &[
            ("facing", "north"),
            ("lit", "true"),
            ("signal_fire", "false"),
            ("waterlogged", "false"),
        ],
        replace: &[FACING_NSWE, LIT],
        &[("signal_fire", Bool), WATERLOGGED],
        opaque = false
    )?;
    register!(
        "soul_campfire",
        override_default: &[
            ("facing", "north"),
            ("lit", "true"),
            ("signal_fire", "false"),
            ("waterlogged", "false"),
        ],
        replace: &[FACING_NSWE, LIT],
        &[("signal_fire", Bool), WATERLOGGED],
        opaque = false
    )?;
    register!("sweet_berry_bush", opaque = false)?;
    register!("warped_stem", override_default: &[("axis", "y")])?;
    register!("stripped_warped_stem", override_default: &[("axis", "y")])?;
    register!("warped_hyphae", override_default: &[("axis", "y")])?;
    register!("stripped_warped_hyphae", override_default: &[("axis", "y")])?;
    register!("warped_nylium")?;
    register!("warped_fungus")?;
    register!("warped_wart_block")?;
    register!("warped_roots")?;
    register!("nether_sprouts")?;
    register!("crimson_stem", override_default: &[("axis", "y")])?;
    register!("stripped_crimson_stem", override_default: &[("axis", "y")])?;
    register!("crimson_hyphae", override_default: &[("axis", "y")])?;
    register!("stripped_crimson_hyphae", override_default: &[("axis", "y")])?;
    register!("crimson_nylium")?;
    register!("crimson_fungus")?;
    register!("shroomlight")?;
    register!("weeping_vines", &[AGE_0_25], opaque = false)?;
    register!("weeping_vines_plant", opaque = false)?;
    register!("twisting_vines", &[AGE_0_25], opaque = false)?;
    register!("twisting_vines_plant", opaque = false)?;
    register!("crimson_roots")?;
    register!("crimson_planks")?;
    register!("warped_planks")?;
    register_slab!("crimson_slab")?;
    register_slab!("warped_slab")?;
    register_pressure_plate!("crimson_pressure_plate")?;
    register_pressure_plate!("warped_pressure_plate")?;
    register_fence!("crimson_fence")?;
    register_fence!("warped_fence")?;
    register_trapdoor!("crimson_trapdoor")?;
    register_trapdoor!("warped_trapdoor")?;
    register_fence_gate!("crimson_fence_gate")?;
    register_fence_gate!("warped_fence_gate")?;
    register_stairs!("crimson_stairs")?;
    register_stairs!("warped_stairs")?;
    register_button!("crimson_button")?;
    register_button!("warped_button")?;
    register_door!("crimson_door")?;
    register_door!("warped_door")?;
    register_sign!("crimson_sign")?;
    register_sign!("warped_sign")?;
    register_wall_sign!("crimson_wall_sign")?;
    register_wall_sign!("warped_wall_sign")?;
    register!(
        "structure_block",
        override_default: &[("mode", "load")],
        replace: &[("mode", Enum(&["save", "load", "corner", "data"]))]
    )?;
    register!(
        "jigsaw",
        override_default: &[("orientation", "north_up")],
        replace: &[("orientation", Enum(&[
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
        ]))]
    )?;
    register!("composter", &[("level", Int(0..=8))], opaque = false)?;
    register!("target", &[("power", Int(0..=15))])?;
    register!("bee_nest", replace: &[FACING_NSWE, ("honey_level", Int(0..=5))])?;
    register!("beehive", replace: &[FACING_NSWE, ("honey_level", Int(0..=5))])?;
    register!("honey_block", opaque = false)?;
    register!("honeycomb_block")?;
    register!("netherite_block")?;
    register!("ancient_debris")?;
    register!("crying_obsidian")?;
    register!("respawn_anchor")?;
    register!("potted_crimson_fungus", opaque = false)?;
    register!("potted_warped_fungus", opaque = false)?;
    register!("potted_crimson_roots", opaque = false)?;
    register!("potted_warped_roots", opaque = false)?;
    register!("lodestone")?;
    register!("blackstone")?;
    register_stairs!("blackstone_stairs")?;
    register_wall!("blackstone_wall")?;
    register_slab!("blackstone_slab")?;
    register!("polished_blackstone")?;
    register!("polished_blackstone_bricks")?;
    register!("cracked_polished_blackstone_bricks")?;
    register!("chiseled_polished_blackstone")?;
    register_slab!("polished_blackstone_brick_slab")?;
    register_stairs!("polished_blackstone_brick_stairs")?;
    register_wall!("polished_blackstone_brick_wall")?;
    register!("gilded_blackstone")?;
    register_stairs!("polished_blackstone_stairs")?;
    register_slab!("polished_blackstone_slab")?;
    register_pressure_plate!("polished_blackstone_pressure_plate")?;
    register_button!("polished_blackstone_button")?;
    register_wall!("polished_blackstone_wall")?;
    register!("chiseled_nether_bricks")?;
    register!("cracked_nether_bricks")?;
    register!("quartz_bricks")?;
    {
        macro_rules! register_candle {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[
                        ("candles", "1"),
                        ("lit", "false"),
                        ("waterlogged", "false"),
                    ],
                    replace: &[("candles", Int(1..=4)), LIT],
                    &[WATERLOGGED],
                    opaque = false
                )
            };
        }
        register_candle!("candle")?;
        register_candle!("white_candle")?;
        register_candle!("orange_candle")?;
        register_candle!("magenta_candle")?;
        register_candle!("light_blue_candle")?;
        register_candle!("yellow_candle")?;
        register_candle!("lime_candle")?;
        register_candle!("pink_candle")?;
        register_candle!("gray_candle")?;
        register_candle!("light_gray_candle")?;
        register_candle!("cyan_candle")?;
        register_candle!("purple_candle")?;
        register_candle!("blue_candle")?;
        register_candle!("brown_candle")?;
        register_candle!("green_candle")?;
        register_candle!("red_candle")?;
        register_candle!("black_candle")?;
    }
    {
        macro_rules! register_candle_cake {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("lit", "false")],
                    replace: &[LIT],
                    opaque = false
                )
            };
        }
        register_candle_cake!("candle_cake")?;
        register_candle_cake!("white_candle_cake")?;
        register_candle_cake!("orange_candle_cake")?;
        register_candle_cake!("magenta_candle_cake")?;
        register_candle_cake!("light_blue_candle_cake")?;
        register_candle_cake!("yellow_candle_cake")?;
        register_candle_cake!("lime_candle_cake")?;
        register_candle_cake!("pink_candle_cake")?;
        register_candle_cake!("gray_candle_cake")?;
        register_candle_cake!("light_gray_candle_cake")?;
        register_candle_cake!("cyan_candle_cake")?;
        register_candle_cake!("purple_candle_cake")?;
        register_candle_cake!("blue_candle_cake")?;
        register_candle_cake!("brown_candle_cake")?;
        register_candle_cake!("green_candle_cake")?;
        register_candle_cake!("red_candle_cake")?;
        register_candle_cake!("black_candle_cake")?;
    }
    register!("amethyst_block")?;
    register!("budding_amethyst")?;
    register!(
        "amethyst_cluster",
        override_default: &[("facing", "up"), ("waterlogged", "false")],
        replace: &[FACING_NESWUD],
        &[WATERLOGGED],
        opaque = false
    )?;
    {
        macro_rules! register_amethyst_bud {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("facing", "up"), ("waterlogged", "false")],
                    replace: &[FACING_NESWUD],
                    &[WATERLOGGED],
                    opaque = false
                )
            };
        }
        register_amethyst_bud!("large_amethyst_bud")?;
        register_amethyst_bud!("medium_amethyst_bud")?;
        register_amethyst_bud!("small_amethyst_bud")?;
    }
    register!("tuff")?;
    register_slab!("tuff_slab")?;
    register_stairs!("tuff_stairs")?;
    register_wall!("tuff_wall")?;
    register!("polished_tuff")?;
    register_slab!("polished_tuff_slab")?;
    register_stairs!("polished_tuff_stairs")?;
    register_wall!("polished_tuff_wall")?;
    register!("chiseled_tuff")?;
    register!("tuff_bricks")?;
    register_slab!("tuff_brick_slab")?;
    register_stairs!("tuff_brick_stairs")?;
    register_wall!("tuff_brick_wall")?;
    register!("chiseled_tuff_bricks")?;
    register!("calcite")?;
    register!("tinted_glass", opaque = false)?;
    register!("powder_snow")?;
    register!(
        "sculk_sensor",
        override_default: &[
            ("power", "0"),
            ("sculk_sensor_phase", "inactive"),
            ("waterlogged", "false"),
        ],
        full_custom: &[
            ("power", Int(0..=15)),
            ("sculk_sensor_phase", Enum(&["inactive", "active", "cooldown"])),
            WATERLOGGED,
        ],
        skips: &["power", "waterlogged"],
        opaque = false
    )?;
    register!(
        "calibrated_sculk_sensor",
        override_default: &[
            ("facing", "north"),
            ("power", "0"),
            ("sculk_sensor_phase", "inactive"),
            ("waterlogged", "false"),
        ],
        full_custom: &[
            FACING_NSWE,
            ("power", Int(0..=15)),
            ("sculk_sensor_phase", Enum(&["inactive", "active", "cooldown"])),
            WATERLOGGED,
        ],
        skips: &["power", "waterlogged"],
        opaque = false
    )?;
    register!("sculk")?;
    register!(
        "sculk_vein",
        override_default: &[
            ("down", "false"),
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("up", "false"),
            ("west", "false"),
            ("waterlogged", "false"),
        ],
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            WATERLOGGED,
            ("west", Bool),
        ],
        opaque = false
    )?;
    register!(
        "sculk_catalyst",
        override_default: &[("bloom", "false")],
        replace: &[("bloom", Bool)]
    )?;
    register!(
        "sculk_shrieker",
        override_default: &[
            ("can_summon", "false"),
            ("shrieking", "false"),
            ("waterlogged", "false"),
        ],
        replace: &[("can_summon", Bool)],
        &[("shrieking", Bool), WATERLOGGED],
        opaque = false
    )?;
    register!("copper_block")?;
    register!("exposed_copper")?;
    register!("weathered_copper")?;
    register!("oxidized_copper")?;
    register!("copper_ore")?;
    register!("deepslate_copper_ore")?;
    register!("oxidized_cut_copper")?;
    register!("weathered_cut_copper")?;
    register!("exposed_cut_copper")?;
    register!("cut_copper")?;
    register!("oxidized_chiseled_copper")?;
    register!("weathered_chiseled_copper")?;
    register!("exposed_chiseled_copper")?;
    register!("chiseled_copper")?;
    register!("waxed_oxidized_chiseled_copper")?;
    register!("waxed_weathered_chiseled_copper")?;
    register!("waxed_exposed_chiseled_copper")?;
    register!("waxed_chiseled_copper")?;
    register_stairs!("oxidized_cut_copper_stairs")?;
    register_stairs!("weathered_cut_copper_stairs")?;
    register_stairs!("exposed_cut_copper_stairs")?;
    register_stairs!("cut_copper_stairs")?;
    register_slab!("oxidized_cut_copper_slab")?;
    register_slab!("weathered_cut_copper_slab")?;
    register_slab!("exposed_cut_copper_slab")?;
    register_slab!("cut_copper_slab")?;
    register!("waxed_copper_block")?;
    register!("waxed_weathered_copper")?;
    register!("waxed_exposed_copper")?;
    register!("waxed_oxidized_copper")?;
    register!("waxed_oxidized_cut_copper")?;
    register!("waxed_weathered_cut_copper")?;
    register!("waxed_exposed_cut_copper")?;
    register!("waxed_cut_copper")?;
    register_stairs!("waxed_oxidized_cut_copper_stairs")?;
    register_stairs!("waxed_weathered_cut_copper_stairs")?;
    register_stairs!("waxed_exposed_cut_copper_stairs")?;
    register_stairs!("waxed_cut_copper_stairs")?;
    register_slab!("waxed_oxidized_cut_copper_slab")?;
    register_slab!("waxed_weathered_cut_copper_slab")?;
    register_slab!("waxed_exposed_cut_copper_slab")?;
    register_slab!("waxed_cut_copper_slab")?;
    register_door!("copper_door")?;
    register_door!("exposed_copper_door")?;
    register_door!("oxidized_copper_door")?;
    register_door!("weathered_copper_door")?;
    register_door!("waxed_copper_door")?;
    register_door!("waxed_exposed_copper_door")?;
    register_door!("waxed_oxidized_copper_door")?;
    register_door!("waxed_weathered_copper_door")?;
    register_trapdoor!("copper_trapdoor")?;
    register_trapdoor!("exposed_copper_trapdoor")?;
    register_trapdoor!("oxidized_copper_trapdoor")?;
    register_trapdoor!("weathered_copper_trapdoor")?;
    register_trapdoor!("waxed_copper_trapdoor")?;
    register_trapdoor!("waxed_exposed_copper_trapdoor")?;
    register_trapdoor!("waxed_oxidized_copper_trapdoor")?;
    register_trapdoor!("waxed_weathered_copper_trapdoor")?;
    register_grate!("copper_grate")?;
    register_grate!("exposed_copper_grate")?;
    register_grate!("weathered_copper_grate")?;
    register_grate!("oxidized_copper_grate")?;
    register_grate!("waxed_copper_grate")?;
    register_grate!("waxed_exposed_copper_grate")?;
    register_grate!("waxed_weathered_copper_grate")?;
    register_grate!("waxed_oxidized_copper_grate")?;
    {
        macro_rules! register_bulb {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    override_default: &[("lit", "false"), ("powered", "false")],
                    replace: &[LIT, POWERED]
                )
            };
        }
        register_bulb!("copper_bulb")?;
        register_bulb!("exposed_copper_bulb")?;
        register_bulb!("weathered_copper_bulb")?;
        register_bulb!("oxidized_copper_bulb")?;
        register_bulb!("waxed_copper_bulb")?;
        register_bulb!("waxed_exposed_copper_bulb")?;
        register_bulb!("waxed_weathered_copper_bulb")?;
        register_bulb!("waxed_oxidized_copper_bulb")?;
    }
    register!(
        "lightning_rod",
        override_default: &[("facing", "up"), ("powered", "false"), ("waterlogged", "false")],
        replace: &[FACING_NESWUD, POWERED],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "pointed_dripstone",
        override_default: &[
            ("thickness", "tip"),
            ("vertical_direction", "up"),
            ("waterlogged", "false")
        ],
        replace: &[
            ("thickness", Enum(&["tip_merge", "tip", "frustum", "middle", "base"])),
            ("vertical_direction", Enum(&["up", "down"])),
        ],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!("dripstone_block")?;
    // FIXME Again, wrong order
    register!(
        "cave_vines",
        override_default: &[("age", "0"), ("berries", "false")],
        full_custom: &[AGE_0_25, ("berries", Bool)],
        skips: &["age"],
        opaque = false
    )?;
    register!(
        "cave_vines_plant",
        override_default: &[("berries", "false")],
        replace: &[("berries", Bool)],
        opaque = false
    )?;
    register!("spore_blossom", opaque = false)?;
    register!("azalea", opaque = false)?;
    register!("flowering_azalea", opaque = false)?;
    register!("moss_carpet", opaque = false)?;
    register!(
        "pink_petals",
        &[FACING_NSWE, ("flower_amount", Int(1..=4))],
        opaque = false
    )?;
    register!("moss_block", opaque = false)?;
    register!(
        "big_dripleaf",
        override_default: &[("facing", "north"), ("tilt", "none"), ("waterlogged", "false")],
        replace: &[FACING_NSWE, ("tilt", Enum(&["none", "unstable", "partial", "full"]))],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "big_dripleaf_stem",
        override_default: &[("facing", "north"), ("waterlogged", "false")],
        replace: &[FACING_NSWE],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!(
        "small_dripleaf",
        override_default: &[("facing", "north"), ("half", "lower"), ("waterlogged", "false")],
        replace: &[FACING_NSWE, ("half", Enum(&["upper", "lower"]))],
        &[WATERLOGGED],
        opaque = false
    )?;
    register!("hanging_roots", override_default: &[("waterlogged", "false")], &[WATERLOGGED])?;
    register!("rooted_dirt")?;
    register!("mud")?;
    register!("deepslate", override_default: &[("axis", "y")])?;
    register!("cobbled_deepslate")?;
    register_stairs!("cobbled_deepslate_stairs")?;
    register_slab!("cobbled_deepslate_slab")?;
    register_wall!("cobbled_deepslate_wall")?;
    register!("polished_deepslate")?;
    register_stairs!("polished_deepslate_stairs")?;
    register_slab!("polished_deepslate_slab")?;
    register_wall!("polished_deepslate_wall")?;
    register!("deepslate_tiles")?;
    register_stairs!("deepslate_tile_stairs")?;
    register_slab!("deepslate_tile_slab")?;
    register_wall!("deepslate_tile_wall")?;
    register!("deepslate_bricks")?;
    register_stairs!("deepslate_brick_stairs")?;
    register_slab!("deepslate_brick_slab")?;
    register_wall!("deepslate_brick_wall")?;
    register!("chiseled_deepslate")?;
    register!("cracked_deepslate_bricks")?;
    register!("cracked_deepslate_tiles")?;
    register!("infested_deepslate", override_default: &[("axis", "y")])?;
    register!("smooth_basalt")?;
    register!("raw_iron_block")?;
    register!("raw_copper_block")?;
    register!("raw_gold_block")?;
    register!("potted_azalea_bush", opaque = false)?;
    register!("potted_flowering_azalea_bush", opaque = false)?;
    register!("ochre_froglight", override_default: &[("axis", "y")])?;
    register!("verdant_froglight", override_default: &[("axis", "y")])?;
    register!("pearlescent_froglight", override_default: &[("axis", "y")])?;
    register!("frogspawn", opaque = false)?;
    register!("reinforced_deepslate")?;
    register!(
        "decorated_pot",
        override_default: &[("cracked", "false"), ("facing", "north"), ("waterlogged", "false")],
        &[("cracked", Bool), FACING_NSWE, WATERLOGGED],
        opaque = false
    )?;
    register!(
        "crafter",
        override_default: &[
            ("crafting", "false"),
            ("orientation", "north_up"),
            ("triggered", "false"),
        ],
        replace: &[
            ("crafting", Bool),
            ("orientation", Enum(&[
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
            ])),
            ("triggered", Bool),
        ]
    )?;
    register!(
        "trial_spawner",
        replace: &[("trial_spawner_state", Enum(&[
            "inactive",
            "waiting_for_players",
            "active",
            "waiting_for_reward_ejection",
            "ejecting_reward",
            "cooldown",
        ]))],
        opaque = false
    )?;

    println!("Time taken: {:?}", std::time::Instant::now() - start_time);

    // TODO: Move this to a startup flag
    // Write out registry IDs to "entries.json", helpful for adding new blocks
    #[cfg(debug_assertions)]
    {
        use std::fmt::Write;
        let mut block_ids = Vec::new();
        for (identifier, idx) in registry.data.identifier_map.iter() {
            let entry = &registry.data.entries[idx.0 as usize];
            block_ids.push((identifier, entry.blockstate_id_range.clone()));
        }
        block_ids.sort_by_key(|(_, range)| *range.start());
        let last_block = block_ids.len() - 1;
        let mut entries_string = String::from("[\n");
        for (i, (identifier, range)) in block_ids.into_iter().enumerate() {
            if i < last_block {
                writeln!(&mut entries_string, "  \"{identifier}: {range:?}\",")?;
            } else {
                writeln!(&mut entries_string, "  \"{identifier}: {range:?}\"")?;
            }
        }
        write!(&mut entries_string, "]")?;
        std::fs::write("entries.json", entries_string)?;
    }
    // Write out registry blockstate information to "blocks_rust.json".
    // Helpful for adding new blocks
    #[cfg(debug_assertions)]
    {
        use indexmap::{IndexMap, IndexSet};
        #[derive(Clone, Debug, serde::Serialize)]
        struct JsonBlock {
            #[serde(skip_serializing_if = "IndexMap::is_empty")]
            pub properties: IndexMap<Atom, IndexSet<Atom>>,
            pub states: Vec<JsonBlockstate>,
        }
        #[derive(Clone, Debug, serde::Serialize)]
        struct JsonBlockstate {
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            pub default: bool,
            pub id: usize,
            #[serde(skip_serializing_if = "IndexMap::is_empty")]
            pub properties: IndexMap<Atom, Atom>,
        }
        // The Minecraft data generators sometimes break alphabetical order (why?) for properties,
        // so fix property sort order here for blocks which need it.
        let custom_ordering_map = {
            let mut map = AHashMap::new();
            macro_rules! insert_entry {
                ($identifier:expr, [ $( $name:expr ),+ ]) => {
                    map.insert(crate::identifier!($identifier), vec![$( Atom::from($name), )+])
                };
            }
            insert_entry!("chest", ["type", "facing", "waterlogged"]);
            insert_entry!("moving_piston", ["type", "facing"]);
            insert_entry!("piston_head", ["type", "facing", "short"]);
            insert_entry!("trapped_chest", ["type", "facing", "waterlogged"]);
            map
        };
        let mut blocks = IndexMap::new();
        for (id, info) in registry.global_palette.iter().enumerate() {
            let identifier = registry.get_identifier_from_index(info.block_index).unwrap();
            let block_entry = blocks
                .entry(identifier.to_string())
                .or_insert(JsonBlock {
                    properties: IndexMap::new(),
                    states: Vec::new(),
                });
            let mut properties: IndexMap<_, _> = info
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if let Some(custom_ordering) = custom_ordering_map.get(&identifier) {
                properties.sort_by_cached_key(|n1, _| {
                    custom_ordering.iter().position(|n2| n1 == n2).unwrap()
                });
            } else {
                properties.sort_unstable_keys();
            }
            for (property, value) in properties.iter() {
                let property_entry = block_entry
                    .properties
                    .entry(property.clone())
                    .or_insert(IndexSet::new());
                property_entry.insert(value.clone());
            }
            let default = registry
                .data
                [info.block_index]
                .default_blockstate
                .as_usize()
                == id;
            block_entry.states.push(JsonBlockstate {
                default,
                id,
                properties,
            });
        }
        blocks.sort_unstable_keys();
        let blocks_string = serde_json::to_string_pretty(&blocks)?;
        std::fs::write("blocks_rust.json", blocks_string)?;
    }

    Ok(())
}
