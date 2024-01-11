pub mod blockstate;
pub mod model;

use super::{texture, Identifier, RegistryData, RegistryIndex};
use crate::client::graphics::chunk::block_face::rotation_matrices;
use anyhow::Context;
use blockstate::CustomPropertyType;
use model::ModelCache;
use serde_repr::Deserialize_repr;

#[derive(Debug)]
pub struct Registry {
    data: RegistryData<Info>,
    pub global_palette: Vec<blockstate::Blockstate>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalPaletteIndex(u16);

impl From<GlobalPaletteIndex> for usize {
    fn from(value: GlobalPaletteIndex) -> Self {
        value.as_usize()
    }
}

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
        properties: Properties,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<RegistryIndex> {
        let mut blockstates = blockstate::load_blockstates(
            &identifier,
            custom_variant_properties,
            model_cache,
            texture_atlas,
        )
        .with_context(|| format!("Failed to parse blockstates for {identifier:?}"))?;
        #[cfg(debug_assertions)]
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        // FIXME This isn't correct, see "todo.txt"
        let default_index = self.global_palette.len();
        self.global_palette.append(&mut blockstates);
        let info = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range,
        };
        // TODO Not all blockstates are getting generated, so tack on some extra debugging info for
        // blockstate groups and conditions. Probably make those active when compiled as a debug
        // build.
        Ok(self.data.register(identifier, info))
    }

    pub fn register_liquid(
        &mut self,
        identifier: Identifier,
        properties: Properties,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<RegistryIndex> {
        let mut blockstates =
            blockstate::load_liquid_blockstates(&identifier, model_cache, texture_atlas)
                .with_context(|| format!("while parsing liquid blockstates for {identifier:?}"))?;
        #[cfg(debug_assertions)]
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = self.global_palette.len();
        self.global_palette.append(&mut blockstates);
        let info = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range,
        };
        Ok(self.data.register(identifier, info))
    }

    pub fn get_entry_from_identifer(&self, identifier: &Identifier) -> Option<&Info> {
        self.data.get_entry_from_identifer(identifier)
    }
}

#[derive(Debug)]
pub struct Info {
    pub default_blockstate: GlobalPaletteIndex,
    pub properties: Properties,
    #[cfg(debug_assertions)]
    pub blockstate_id_range: std::ops::RangeInclusive<usize>,
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

// TODO Write some `register_vanilla_blocks` function and just start importing a bunch
// TODO Make model loading errors print out a warning and load missing texture block instead

pub fn register_vanilla_blocks(
    registry: &mut Registry,
    model_cache: &mut ModelCache,
    texture_atlas_builder: &mut texture::AtlasBuilder,
) -> anyhow::Result<()> {
    use CustomPropertyType::*;
    macro_rules! register {
        ($identifier:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $( $key:ident = $value:expr ),+) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                None,
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $custom_variants:expr) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Properties::default(),
                model_cache,
                texture_atlas_builder,
            )
        };
        ($identifier:expr, $custom_variants:expr, $( $key:ident = $value:expr ),+) => {
            registry.register(
                Identifier::parse($identifier).unwrap(),
                Some($custom_variants),
                Properties {
                    $( $key: $value ),+,
                    ..Default::default()
                },
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
    const AGE_0_15: (&str, blockstate::CustomPropertyType) = ("age", Int(0..=15));
    const AGE_0_25: (&str, blockstate::CustomPropertyType) = ("age", Int(0..=25));
    const ROTATION_0_15: (&str, blockstate::CustomPropertyType) = ("rotation", Int(0..=15));
    const CHEST_TYPE: (&str, blockstate::CustomPropertyType) =
        ("type", Enum(&["single", "left", "right"]));

    let start_time = std::time::Instant::now();

    // Specialised registration macros for common block types
    macro_rules! register_slab {
        ($identifier:expr) => {
            register!($identifier, &[WATERLOGGED])
        };
    }
    macro_rules! register_stairs {
        ($identifier:expr) => {
            register!($identifier, &[WATERLOGGED])
        };
    }
    macro_rules! register_fence {
        ($identifier:expr) => {
            register!(
                $identifier,
                &[
                    ("east", Bool),
                    ("north", Bool),
                    ("south", Bool),
                    WATERLOGGED,
                    ("west", Bool),
                ]
            )
        };
    }
    macro_rules! register_fence_gate {
        ($identifier:expr) => {
            register!($identifier, &[POWERED])
        };
    }
    macro_rules! register_wall {
        ($identifier:expr) => {
            register!(
                $identifier,
                &[
                    ("east", Enum(&["none", "low", "tall"])),
                    ("north", Enum(&["none", "low", "tall"])),
                    ("south", Enum(&["none", "low", "tall"])),
                    ("up", Bool),
                    WATERLOGGED,
                    ("west", Enum(&["none", "low", "tall"])),
                ]
            )
        };
    }
    macro_rules! register_door {
        ($identifier:expr) => {
            register!($identifier, &[POWERED])
        };
    }
    macro_rules! register_trapdoor {
        ($identifier:expr) => {
            register!($identifier, &[POWERED, WATERLOGGED])
        };
    }
    macro_rules! register_chest {
        ($identifier:expr) => {
            register!($identifier, &[CHEST_TYPE, FACING_NSWE, WATERLOGGED])
        };
    }
    macro_rules! register_sign {
        ($identifier:expr) => {
            register!($identifier, &[ROTATION_0_15, WATERLOGGED,])
        };
    }
    macro_rules! register_wall_sign {
        ($identifier:expr) => {
            register!($identifier, &[FACING_NSWE, WATERLOGGED,])
        };
    }
    macro_rules! register_hanging_sign {
        ($identifier:expr) => {
            register!(
                $identifier,
                &[("attached", Bool), ROTATION_0_15, WATERLOGGED,]
            )
        };
    }
    macro_rules! register_wall_hanging_sign {
        ($identifier:expr) => {
            register!($identifier, &[FACING_NSWE, WATERLOGGED,])
        };
    }
    macro_rules! register_stained_glass {
        ($identifier:expr) => {
            register!($identifier, opaque = false)
        };
    }

    register!("air")?;
    register!("stone")?;
    register!("granite")?;
    register!("polished_granite")?;
    register!("diorite")?;
    register!("polished_diorite")?;
    register!("andesite")?;
    register!("polished_andesite")?;
    register!("grass_block")?;
    register!("dirt")?;
    register!("coarse_dirt")?;
    register!("podzol")?;
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
    register!("oak_sapling", &[("stage", Int(0..=1))])?;
    register!("spruce_sapling", &[("stage", Int(0..=1))])?;
    register!("birch_sapling", &[("stage", Int(0..=1))])?;
    register!("jungle_sapling", &[("stage", Int(0..=1))])?;
    register!("acacia_sapling", &[("stage", Int(0..=1))])?;
    register!("cherry_sapling", &[("stage", Int(0..=1))])?;
    register!("dark_oak_sapling", &[("stage", Int(0..=1))])?;
    register!("mangrove_propagule", &[("stage", Int(0..=1)), WATERLOGGED])?;
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
    register!("oak_log")?;
    register!("spruce_log")?;
    register!("birch_log")?;
    register!("jungle_log")?;
    register!("acacia_log")?;
    register!("cherry_log")?;
    register!("dark_oak_log")?;
    register!("mangrove_log")?;
    register!("mangrove_roots", &[WATERLOGGED])?;
    register!("muddy_mangrove_roots")?;
    register!("bamboo_block")?;
    register!("stripped_spruce_log")?;
    register!("stripped_birch_log")?;
    register!("stripped_jungle_log")?;
    register!("stripped_acacia_log")?;
    register!("stripped_cherry_log")?;
    register!("stripped_dark_oak_log")?;
    register!("stripped_oak_log")?;
    register!("stripped_mangrove_log")?;
    register!("stripped_bamboo_block")?;
    register!("oak_wood")?;
    register!("spruce_wood")?;
    register!("birch_wood")?;
    register!("jungle_wood")?;
    register!("acacia_wood")?;
    register!("cherry_wood")?;
    register!("dark_oak_wood")?;
    register!("mangrove_wood")?;
    register!("stripped_oak_wood")?;
    register!("stripped_spruce_wood")?;
    register!("stripped_birch_wood")?;
    register!("stripped_jungle_wood")?;
    register!("stripped_acacia_wood")?;
    register!("stripped_cherry_wood")?;
    register!("stripped_dark_oak_wood")?;
    register!("stripped_mangrove_wood")?;
    {
        macro_rules! register_leaves {
            ($identifier:expr) => {
                register!(
                    $identifier,
                    &[("distance", Int(1..=7)), ("persistent", Bool), WATERLOGGED,]
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
    register!("dispenser", &[("triggered", Bool)])?;
    register!("sandstone")?;
    register!("chiseled_sandstone")?;
    register!("cut_sandstone")?;
    register!(
        "note_block",
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
    register!("white_bed")?;
    register!("orange_bed")?;
    register!("magenta_bed")?;
    register!("light_blue_bed")?;
    register!("yellow_bed")?;
    register!("lime_bed")?;
    register!("pink_bed")?;
    register!("gray_bed")?;
    register!("light_gray_bed")?;
    register!("cyan_bed")?;
    register!("purple_bed")?;
    register!("blue_bed")?;
    register!("brown_bed")?;
    register!("green_bed")?;
    register!("red_bed")?;
    register!("black_bed")?;
    register!("powered_rail", &[WATERLOGGED])?;
    register!("detector_rail", &[WATERLOGGED])?;
    register!("sticky_piston")?;
    register!("cobweb")?;
    register!("grass")?;
    register!("fern")?;
    register!("dead_bush")?;
    register!("seagrass")?;
    register!("tall_seagrass")?;
    register!("piston")?;
    register!("piston_head")?;
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
        &[("type", Enum(&["normal", "sticky"])), FACING_NESWUD,]
    )?;
    register!("dandelion")?;
    register!("torchflower")?;
    register!("poppy")?;
    register!("blue_orchid")?;
    register!("allium")?;
    register!("azure_bluet")?;
    register!("red_tulip")?;
    register!("orange_tulip")?;
    register!("white_tulip")?;
    register!("pink_tulip")?;
    register!("oxeye_daisy")?;
    register!("cornflower")?;
    register!("wither_rose")?;
    register!("lily_of_the_valley")?;
    register!("brown_mushroom")?;
    register!("red_mushroom")?;
    register!("gold_block")?;
    register!("iron_block")?;
    register!("bricks")?;
    register!("tnt", &[("unstable", Bool)])?;
    register!("bookshelf")?;
    register!(
        "chiseled_bookshelf",
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
    register!("torch")?;
    register!("wall_torch")?;
    register!(
        "fire",
        &[
            AGE_0_15,
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ]
    )?;
    register!("soul_fire", &[])?;
    register!("spawner", opaque = false)?;
    register_stairs!("oak_stairs")?;
    register_chest!("chest")?;
    register!(
        "redstone_wire",
        &[
            ("east", Enum(&["up", "side", "none"])),
            ("north", Enum(&["up", "side", "none"])),
            ("power", Int(0..=15)),
            ("south", Enum(&["up", "side", "none"])),
            ("west", Enum(&["up", "side", "none"])),
        ]
    )?;
    register!("diamond_ore")?;
    register!("deepslate_diamond_ore")?;
    register!("diamond_block")?;
    register!("crafting_table")?;
    register!("wheat")?;
    register!("farmland")?;
    register!("furnace")?;
    register_sign!("oak_sign")?;
    register_sign!("spruce_sign")?;
    register_sign!("birch_sign")?;
    register_sign!("acacia_sign")?;
    register_sign!("cherry_sign")?;
    register_sign!("jungle_sign")?;
    register_sign!("dark_oak_sign")?;
    register_sign!("mangrove_sign")?;
    register_sign!("bamboo_sign")?;
    register!("oak_door", &[POWERED])?;
    register!("ladder", &[WATERLOGGED])?;
    register!("rail", &[WATERLOGGED])?;
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
    register!("lever")?;
    register!("stone_pressure_plate")?;
    register!("iron_door", &[POWERED])?;
    register!("oak_pressure_plate")?;
    register!("spruce_pressure_plate")?;
    register!("birch_pressure_plate")?;
    register!("jungle_pressure_plate")?;
    register!("acacia_pressure_plate")?;
    register!("cherry_pressure_plate")?;
    register!("dark_oak_pressure_plate")?;
    register!("mangrove_pressure_plate")?;
    register!("bamboo_pressure_plate")?;
    register!("redstone_ore", &[("lit", Bool)])?;
    register!("deepslate_redstone_ore", &[("lit", Bool)])?;
    register!("redstone_torch")?;
    register!("redstone_wall_torch")?;
    register!("stone_button")?;
    register!("snow")?;
    register!("ice")?;
    register!("snow_block")?;
    register!("cactus", &[AGE_0_15])?;
    register!("clay")?;
    register!("sugar_cane", &[AGE_0_15])?;
    register!("jukebox", &[("has_record", Bool)])?;
    register!(
        "oak_fence",
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            WATERLOGGED,
            ("west", Bool),
        ]
    )?;
    register!("pumpkin")?;
    register!("netherrack")?;
    register!("soul_sand")?;
    register!("soul_soil")?;
    register!("basalt")?;
    register!("polished_basalt")?;
    register!("soul_torch")?;
    register!("soul_wall_torch")?;
    register!("glowstone")?;
    register!("nether_portal")?;
    register!("carved_pumpkin")?;
    register!("jack_o_lantern")?;
    register!("cake")?;
    register!("repeater")?;
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
    {
        macro_rules! register_trapdoor {
            ($identifier:expr) => {
                register!($identifier, &[POWERED, WATERLOGGED,])
            };
        }
        register_trapdoor!("oak_trapdoor")?;
        register_trapdoor!("spruce_trapdoor")?;
        register_trapdoor!("birch_trapdoor")?;
        register_trapdoor!("jungle_trapdoor")?;
        register_trapdoor!("acacia_trapdoor")?;
        register_trapdoor!("cherry_trapdoor")?;
        register_trapdoor!("dark_oak_trapdoor")?;
        register_trapdoor!("mangrove_trapdoor")?;
        register_trapdoor!("bamboo_trapdoor")?;
    }
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
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            WATERLOGGED,
            ("west", Bool),
        ]
    )?;
    register!("chain", &[WATERLOGGED])?;
    register!(
        "glass_pane",
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            WATERLOGGED,
            ("west", Bool),
        ]
    )?;
    register!("melon")?;
    register!("attached_pumpkin_stem")?;
    register!("attached_melon_stem")?;
    register!("pumpkin_stem")?;
    register!("melon_stem")?;
    register!(
        "vine",
        &[
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ]
    )?;
    register!(
        "glow_lichen",
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            WATERLOGGED,
            ("west", Bool),
        ]
    )?;
    register_fence_gate!("oak_fence_gate")?;
    register_stairs!("brick_stairs")?;
    register_stairs!("stone_brick_stairs")?;
    register_stairs!("mud_brick_stairs")?;
    register!("mycelium")?;
    register!("lily_pad")?;
    register!("nether_bricks")?;
    register_fence!("nether_brick_fence")?;
    register_stairs!("nether_brick_stairs")?;
    register!("nether_wart")?;
    register!("enchanting_table")?;
    register!(
        "brewing_stand",
        &[
            ("has_bottle_0", Bool),
            ("has_bottle_1", Bool),
            ("has_bottle_2", Bool)
        ]
    )?;
    register!("cauldron")?;
    register!("water_cauldron")?;
    register!("lava_cauldron")?;
    register!("powder_snow_cauldron")?;
    register!("end_portal")?;
    register!("end_portal_frame")?;
    register!("end_stone")?;
    register!("dragon_egg")?;
    register!("redstone_lamp")?;
    register!("cocoa")?;
    register_stairs!("sandstone_stairs")?;
    register!("emerald_ore")?;
    register!("deepslate_emerald_ore")?;
    register!("ender_chest", &[FACING_NSWE, WATERLOGGED])?;
    register!("tripwire_hook")?;
    // FIXME Order's completely wrong for this, so just make a property override for variants the
    // same as with multiparts, where we define everything. Panic if combination doesn't have a
    // model.
    register!("tripwire", &[("disarmed", Bool), POWERED])?;
    register!("emerald_block")?;
    register_stairs!("spruce_stairs")?;
    register_stairs!("birch_stairs")?;
    register_stairs!("jungle_stairs")?;
    register!("command_block")?;
    register!("beacon")?;
    register_wall!("cobblestone_wall")?;
    register_wall!("mossy_cobblestone_wall")?;
    register!("flower_pot")?;
    register!("potted_torchflower")?;
    register!("potted_oak_sapling")?;
    register!("potted_spruce_sapling")?;
    register!("potted_birch_sapling")?;
    register!("potted_jungle_sapling")?;
    register!("potted_acacia_sapling")?;
    register!("potted_cherry_sapling")?;
    register!("potted_dark_oak_sapling")?;
    register!("potted_mangrove_propagule")?;
    register!("potted_fern")?;
    register!("potted_dandelion")?;
    register!("potted_poppy")?;
    register!("potted_blue_orchid")?;
    register!("potted_allium")?;
    register!("potted_azure_bluet")?;
    register!("potted_red_tulip")?;
    register!("potted_orange_tulip")?;
    register!("potted_white_tulip")?;
    register!("potted_pink_tulip")?;
    register!("potted_oxeye_daisy")?;
    register!("potted_cornflower")?;
    register!("potted_lily_of_the_valley")?;
    register!("potted_wither_rose")?;
    register!("potted_red_mushroom")?;
    register!("potted_brown_mushroom")?;
    register!("potted_dead_bush")?;
    register!("potted_cactus")?;
    register!("carrots")?;
    register!("potatoes")?;
    register!("oak_button")?;
    register!("spruce_button")?;
    register!("birch_button")?;
    register!("jungle_button")?;
    register!("acacia_button")?;
    register!("cherry_button")?;
    register!("dark_oak_button")?;
    register!("mangrove_button")?;
    register!("bamboo_button")?;
    {
        macro_rules! register_head {
            ($identifier:expr) => {
                register!($identifier, &[ROTATION_0_15])
            };
        }
        macro_rules! register_wall_head {
            ($identifier:expr) => {
                register!($identifier, &[FACING_NSWE])
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
    register!("anvil")?;
    register!("chipped_anvil")?;
    register!("damaged_anvil")?;
    register_chest!("trapped_chest")?;
    register!("light_weighted_pressure_plate")?;
    register!("heavy_weighted_pressure_plate")?;
    register!("comparator")?;
    register!("daylight_detector", &[("power", Int(0..=15))])?;
    register!("redstone_block")?;
    register!("nether_quartz_ore")?;
    register!("hopper", &[("enabled", Bool)])?;
    register!("quartz_block")?;
    register!("chiseled_quartz_block")?;
    register!("quartz_pillar")?;
    register!("quartz_stairs", &[WATERLOGGED])?;
    register!("activator_rail", &[WATERLOGGED])?;
    register!("dropper", &[("triggered", Bool)])?;
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
                    &[
                        ("east", Bool),
                        ("north", Bool),
                        ("south", Bool),
                        WATERLOGGED,
                        ("west", Bool),
                    ]
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
    register!("slime_block")?;
    register!("barrier")?;
    register!("light", &[WATERLOGGED])?;
    register!("iron_trapdoor", &[POWERED, WATERLOGGED])?;
    register!("prismarine")?;
    register!("prismarine_bricks")?;
    register!("dark_prismarine")?;
    register_stairs!("prismarine_stairs")?;
    register_stairs!("prismarine_brick_stairs")?;
    register_stairs!("dark_prismarine_stairs")?;
    register_slab!("prismarine_slab")?;
    register_slab!("prismarine_brick_slab")?;
    register_slab!("dark_prismarine_slab")?;
    register!("sea_lantern")?;
    register!("hay_block")?;
    register!("white_carpet")?;
    register!("orange_carpet")?;
    register!("magenta_carpet")?;
    register!("light_blue_carpet")?;
    register!("yellow_carpet")?;
    register!("lime_carpet")?;
    register!("pink_carpet")?;
    register!("gray_carpet")?;
    register!("light_gray_carpet")?;
    register!("cyan_carpet")?;
    register!("purple_carpet")?;
    register!("blue_carpet")?;
    register!("brown_carpet")?;
    register!("green_carpet")?;
    register!("red_carpet")?;
    register!("black_carpet")?;
    register!("terracotta")?;
    register!("coal_block")?;
    register!("packed_ice")?;
    register!("sunflower")?;
    register!("lilac")?;
    register!("rose_bush")?;
    register!("peony")?;
    register!("tall_grass")?;
    register!("large_fern")?;
    {
        macro_rules! register_banner {
            ($identifier:expr) => {
                register!($identifier, &[ROTATION_0_15])
            };
        }
        macro_rules! register_wall_banner {
            ($identifier:expr) => {
                register!($identifier, &[FACING_NSWE])
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
    register!("end_rod")?;
    register!(
        "chorus_plant",
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            ("west", Bool),
        ]
    )?;
    register!("chorus_flower")?;
    register!("purpur_block")?;
    register!("purpur_pillar")?;
    register_stairs!("purpur_stairs")?;
    register!("end_stone_bricks")?;
    register!("torchflower_crop")?;
    register!("pitcher_crop")?;
    register!("pitcher_plant")?;
    register!("beetroots")?;
    register!("dirt_path")?;
    register!("end_gateway")?;
    register!("repeating_command_block")?;
    register!("chain_command_block")?;
    register!("frosted_ice")?;
    register!("magma_block")?;
    register!("nether_wart_block")?;
    register!("red_nether_bricks")?;
    register!("bone_block")?;
    register!("structure_void")?;
    register!("observer")?;
    {
        macro_rules! register_shulker_box {
            ($identifier:expr) => {
                register!($identifier, &[FACING_NESWUD])
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
    register!("white_glazed_terracotta")?;
    register!("orange_glazed_terracotta")?;
    register!("magenta_glazed_terracotta")?;
    register!("light_blue_glazed_terracotta")?;
    register!("yellow_glazed_terracotta")?;
    register!("lime_glazed_terracotta")?;
    register!("pink_glazed_terracotta")?;
    register!("gray_glazed_terracotta")?;
    register!("light_gray_glazed_terracotta")?;
    register!("cyan_glazed_terracotta")?;
    register!("purple_glazed_terracotta")?;
    register!("blue_glazed_terracotta")?;
    register!("brown_glazed_terracotta")?;
    register!("green_glazed_terracotta")?;
    register!("red_glazed_terracotta")?;
    register!("black_glazed_terracotta")?;
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
    register!("kelp", &[("age", Int(0..=25))])?;
    register!("kelp_plant")?;
    register!("dried_kelp_block")?;
    register!("turtle_egg")?;
    register!("sniffer_egg")?;
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
                register!($identifier, &[WATERLOGGED])
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
        register_coral!("dead_tube_coral_wall_fan")?;
        register_coral!("dead_brain_coral_wall_fan")?;
        register_coral!("dead_bubble_coral_wall_fan")?;
        register_coral!("dead_fire_coral_wall_fan")?;
        register_coral!("dead_horn_coral_wall_fan")?;
        register_coral!("tube_coral_wall_fan")?;
        register_coral!("brain_coral_wall_fan")?;
        register_coral!("bubble_coral_wall_fan")?;
        register_coral!("fire_coral_wall_fan")?;
        register_coral!("horn_coral_wall_fan")?;
    }
    register!("sea_pickle")?;
    register!("blue_ice")?;
    register!("conduit", &[WATERLOGGED])?;
    register!("bamboo_sapling")?;
    register!(
        "bamboo",
        &[
            ("age", Int(0..=1)),
            ("leaves", Enum(&["none", "small", "large"])),
            ("stage", Int(0..=1))
        ]
    )?;
    register!("potted_bamboo")?;
    register!("void_air")?;
    register!("cave_air")?;
    register!("bubble_column", &[("drag", Bool)])?;
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
    register!("scaffolding", &[("distance", Int(0..=7)), WATERLOGGED])?;
    register!("loom")?;
    register!("barrel")?;
    register!("smoker")?;
    register!("blast_furnace")?;
    register!("cartography_table")?;
    register!("fletching_table")?;
    register!("grindstone")?;
    register!("lectern", &[("has_book", Bool), POWERED])?;
    register!("smithing_table")?;
    register!("stonecutter")?;
    register!("bell", &[POWERED])?;
    register!("lantern", &[WATERLOGGED])?;
    register!("soul_lantern", &[WATERLOGGED])?;
    register!("campfire", &[("signal_fire", Bool), WATERLOGGED])?;
    register!("soul_campfire", &[("signal_fire", Bool), WATERLOGGED])?;
    register!("sweet_berry_bush")?;
    register!("warped_stem")?;
    register!("stripped_warped_stem")?;
    register!("warped_hyphae")?;
    register!("stripped_warped_hyphae")?;
    register!("warped_nylium")?;
    register!("warped_fungus")?;
    register!("warped_wart_block")?;
    register!("warped_roots")?;
    register!("nether_sprouts")?;
    register!("crimson_stem")?;
    register!("stripped_crimson_stem")?;
    register!("crimson_hyphae")?;
    register!("stripped_crimson_hyphae")?;
    register!("crimson_nylium")?;
    register!("crimson_fungus")?;
    register!("shroomlight")?;
    register!("weeping_vines", &[AGE_0_25])?;
    register!("weeping_vines_plant")?;
    register!("twisting_vines", &[AGE_0_25])?;
    register!("twisting_vines_plant")?;
    register!("crimson_roots")?;
    register!("crimson_planks")?;
    register!("warped_planks")?;
    register_slab!("crimson_slab")?;
    register_slab!("warped_slab")?;
    register!("crimson_pressure_plate")?;
    register!("warped_pressure_plate")?;
    register_fence!("crimson_fence")?;
    register_fence!("warped_fence")?;
    register_trapdoor!("crimson_trapdoor")?;
    register_trapdoor!("warped_trapdoor")?;
    register_fence_gate!("crimson_fence_gate")?;
    register_fence_gate!("warped_fence_gate")?;
    register_stairs!("crimson_stairs")?;
    register_stairs!("warped_stairs")?;
    register!("crimson_button")?;
    register!("warped_button")?;
    register_door!("crimson_door")?;
    register_door!("warped_door")?;
    register_sign!("crimson_sign")?;
    register_sign!("warped_sign")?;
    register_wall_sign!("crimson_wall_sign")?;
    register_wall_sign!("warped_wall_sign")?;
    register!("structure_block")?;
    register!("jigsaw")?;
    register!("composter", &[("level", Int(0..=8))])?;
    register!("target", &[("power", Int(0..=15))])?;
    register!("bee_nest")?;
    register!("beehive")?;
    register!("honey_block")?;
    register!("honeycomb_block")?;
    register!("netherite_block")?;
    register!("ancient_debris")?;
    register!("crying_obsidian")?;
    register!("respawn_anchor")?;
    register!("potted_crimson_fungus")?;
    register!("potted_warped_fungus")?;
    register!("potted_crimson_roots")?;
    register!("potted_warped_roots")?;
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
    register!("polished_blackstone_pressure_plate")?;
    register!("polished_blackstone_button")?;
    register_wall!("polished_blackstone_wall")?;
    register!("chiseled_nether_bricks")?;
    register!("cracked_nether_bricks")?;
    register!("quartz_bricks")?;
    {
        macro_rules! register_candle {
            ($identifier:expr) => {
                register!($identifier, &[WATERLOGGED])
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
    register!("candle_cake")?;
    register!("white_candle_cake")?;
    register!("orange_candle_cake")?;
    register!("magenta_candle_cake")?;
    register!("light_blue_candle_cake")?;
    register!("yellow_candle_cake")?;
    register!("lime_candle_cake")?;
    register!("pink_candle_cake")?;
    register!("gray_candle_cake")?;
    register!("light_gray_candle_cake")?;
    register!("cyan_candle_cake")?;
    register!("purple_candle_cake")?;
    register!("blue_candle_cake")?;
    register!("brown_candle_cake")?;
    register!("green_candle_cake")?;
    register!("red_candle_cake")?;
    register!("black_candle_cake")?;
    register!("amethyst_block")?;
    register!("budding_amethyst")?;
    register!("amethyst_cluster", &[WATERLOGGED])?;
    register!("large_amethyst_bud", &[WATERLOGGED])?;
    register!("medium_amethyst_bud", &[WATERLOGGED])?;
    register!("small_amethyst_bud", &[WATERLOGGED])?;
    register!("tuff")?;
    register!("calcite")?;
    register!("tinted_glass", opaque = false)?;
    register!("powder_snow")?;
    // FIXME Same as above, completely wrong order
    register!("sculk_sensor", &[("power", Int(0..=15)), WATERLOGGED])?;
    // FIXME And again, wrong order
    register!(
        "calibrated_sculk_sensor",
        &[("power", Int(0..=15)), WATERLOGGED]
    )?;
    register!("sculk")?;
    register!(
        "sculk_vein",
        &[
            ("down", Bool),
            ("east", Bool),
            ("north", Bool),
            ("south", Bool),
            ("up", Bool),
            WATERLOGGED,
            ("west", Bool),
        ]
    )?;
    register!("sculk_catalyst")?;
    register!("sculk_shrieker", &[("shrieking", Bool), WATERLOGGED])?;
    register!("oxidized_copper")?;
    register!("weathered_copper")?;
    register!("exposed_copper")?;
    register!("copper_block")?;
    register!("copper_ore")?;
    register!("deepslate_copper_ore")?;
    register!("oxidized_cut_copper")?;
    register!("weathered_cut_copper")?;
    register!("exposed_cut_copper")?;
    register!("cut_copper")?;
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
    register!("lightning_rod", &[WATERLOGGED])?;
    register!("pointed_dripstone", &[WATERLOGGED])?;
    register!("dripstone_block")?;
    // FIXME Again, wrong order
    register!("cave_vines", &[AGE_0_25])?;
    register!("cave_vines_plant")?;
    register!("spore_blossom")?;
    register!("azalea")?;
    register!("flowering_azalea")?;
    register!("moss_carpet")?;
    register!("pink_petals", &[FACING_NSWE, ("flower_amount", Int(1..=4))])?;
    register!("moss_block")?;
    register!("big_dripleaf", &[WATERLOGGED])?;
    register!("big_dripleaf_stem", &[WATERLOGGED])?;
    register!("small_dripleaf", &[WATERLOGGED])?;
    register!("hanging_roots", &[WATERLOGGED])?;
    register!("rooted_dirt")?;
    register!("mud")?;
    register!("deepslate")?;
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
    register!("infested_deepslate")?;
    register!("smooth_basalt")?;
    register!("raw_iron_block")?;
    register!("raw_copper_block")?;
    register!("raw_gold_block")?;
    register!("potted_azalea_bush")?;
    register!("potted_flowering_azalea_bush")?;
    register!("ochre_froglight")?;
    register!("verdant_froglight")?;
    register!("pearlescent_froglight")?;
    register!("frogspawn")?;
    register!("reinforced_deepslate")?;
    register!(
        "decorated_pot",
        &[("cracked", Bool), FACING_NSWE, WATERLOGGED]
    )?;

    println!("Time taken: {:?}", std::time::Instant::now() - start_time);

    // Write out registry IDs to "entries.txt", helpful for adding new blocks
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
    // log::debug!("Registry: {:#?}", registry.data);

    Ok(())
}
