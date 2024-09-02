pub mod blockstate;
pub mod model;

use super::{texture, Identifier, RegistryData, RegistryIndex};
use ahash::AHashSet;
use anyhow::{anyhow, Context};
use blockstate::{BlockOpacity, BlockstateInfo, BlockstateInfoModifier, CustomPropertyType};
use model::ModelCache;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use string_cache::DefaultAtom as Atom;

#[derive(Debug, Default)]
pub struct Registry {
    data: RegistryData<Info>,
    global_palette: Vec<blockstate::Blockstate>,
    air_blockstates: AHashSet<GlobalPaletteIndex>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GlobalPaletteIndex(u16);

impl GlobalPaletteIndex {
    pub fn placeholder() -> Self {
        Self(0xFFFF)
    }

    pub fn as_raw(&self) -> u16 {
        self.0
    }

    #[inline(always)]
    pub fn as_usize(&self) -> usize {
        self.0 as usize
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

impl Registry {
    pub fn new() -> Self {
        Self {
            data: RegistryData::new(),
            global_palette: Vec::new(),
            air_blockstates: AHashSet::new(),
        }
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

    /// Returns `true` if all of the blockstate belongs to an air-type block.
    /// In vanilla, this is just air, cave air, and void air.
    pub fn is_blockstate_air_like(&self, global_palette_index: GlobalPaletteIndex) -> bool {
        self.air_blockstates.contains(&global_palette_index)
    }

    /// Panics if an entry is already registered with `identifier`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn register<'a, I, II>(
        &mut self,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        custom_variant_properties: Option<&'a [(&'a str, CustomPropertyType)]>,
        replacement_variant_properties: Option<&'a [(&'a str, CustomPropertyType)]>,
        default_override: Option<&'a [(&'a str, &'a str)]>,
        properties: Properties,
        default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone + std::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
    {
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
        // Apply extra information
        let extra_info_modifiers_iter = extra_info_modifiers.into_iter();
        for blockstate in &mut blockstates {
            blockstate.extra_info = default_extra_info;
            'case_loop: for (info_modifier, property_set) in extra_info_modifiers_iter.clone() {
                for (k, v) in property_set {
                    if blockstate.properties[k] != **v {
                        continue 'case_loop;
                    }
                }
                // If all property conditions in a property modifier match, then apply.
                blockstate.extra_info.merge_modifier(info_modifier);
            }
        }
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override
                    .iter()
                    .map(|(k, v)| (Atom::from(*k), Atom::from(*v)))
                    .collect();
                self.global_palette.len()
                    + blockstates
                        .iter()
                        .position(|blockstate| blockstate.properties == override_map)
                        .ok_or(anyhow!("`default_override` should exist as a valid state"))
                        .with_context(|| {
                            format!("Failed to override default blockstate for {identifier:?}")
                        })?
            }
            None => self.global_palette.len(),
        };
        self.global_palette.append(&mut blockstates);
        self.data[block_index] = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range: blockstate_id_range.clone(),
        };
        if properties.air_like {
            for blockstate_id_usize in blockstate_id_range {
                let blockstate_id = GlobalPaletteIndex::try_from(blockstate_id_usize).unwrap();
                self.air_blockstates.insert(blockstate_id);
            }
        }
        Ok(block_index)
    }

    /// Panics if an entry is already registered with `identifier`.
    /// `custom_properties` is each property that defines the blockstates, in order.
    /// `skip_properties` is a list of property names from `properties` that do not appear in the
    /// blockstates file.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn register_full_custom<'a, I, II>(
        &mut self,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        custom_properties: &'a [(&'a str, CustomPropertyType)],
        skip_properties: &'a [&'a str],
        default_override: Option<&'a [(&'a str, &'a str)]>,
        properties: Properties,
        default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone + std::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
    {
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
        // Apply extra information
        let extra_info_modifiers_iter = extra_info_modifiers.into_iter();
        for blockstate in &mut blockstates {
            blockstate.extra_info = default_extra_info;
            'case_loop: for (info_modifier, property_set) in extra_info_modifiers_iter.clone() {
                for (k, v) in property_set {
                    if blockstate.properties[k] != **v {
                        continue 'case_loop;
                    }
                }
                // If all property conditions in a property modifier match, then apply.
                blockstate.extra_info.merge_modifier(info_modifier);
            }
        }
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override
                    .iter()
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
            blockstate_id_range: blockstate_id_range.clone(),
        };
        if properties.air_like {
            for blockstate_id_usize in blockstate_id_range {
                let blockstate_id = GlobalPaletteIndex::try_from(blockstate_id_usize).unwrap();
                self.air_blockstates.insert(blockstate_id);
            }
        }
        Ok(block_index)
    }

    pub(super) fn register_liquid<'a, I, II>(
        &mut self,
        model_cache: &mut ModelCache,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        properties: Properties,
        mut default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone + std::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
    {
        let block_index = self.data.register_default(identifier.clone());
        default_extra_info.opacity = BlockOpacity::Transparent;
        let mut blockstates = blockstate::load_liquid_blockstates(
            block_index,
            &identifier,
            model_cache,
            texture_atlas,
        )
        .with_context(|| format!("while parsing liquid blockstates for {identifier:?}"))?;
        // Apply extra information
        let extra_info_modifiers_iter = extra_info_modifiers.into_iter();
        for blockstate in &mut blockstates {
            blockstate.extra_info = default_extra_info;
            'case_loop: for (info_modifier, property_set) in extra_info_modifiers_iter.clone() {
                for (k, v) in property_set {
                    if blockstate.properties[k] != **v {
                        continue 'case_loop;
                    }
                }
                // If all property conditions in a property modifier match, then apply.
                blockstate.extra_info.merge_modifier(info_modifier);
            }
        }
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        let default_index = self.global_palette.len();
        self.global_palette.append(&mut blockstates);
        self.data[block_index] = Info {
            default_blockstate: GlobalPaletteIndex(default_index.try_into().unwrap()),
            properties,
            #[cfg(debug_assertions)]
            blockstate_id_range: blockstate_id_range.clone(),
        };
        if properties.air_like {
            for blockstate_id_usize in blockstate_id_range {
                let blockstate_id = GlobalPaletteIndex::try_from(blockstate_id_usize).unwrap();
                self.air_blockstates.insert(blockstate_id);
            }
        }
        Ok(block_index)
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

impl std::ops::Index<GlobalPaletteIndex> for Registry {
    type Output = blockstate::Blockstate;

    fn index(&self, index: GlobalPaletteIndex) -> &blockstate::Blockstate {
        &self.global_palette[index.as_usize()]
    }
}

impl std::ops::IndexMut<GlobalPaletteIndex> for Registry {
    fn index_mut(&mut self, index: GlobalPaletteIndex) -> &mut Self::Output {
        &mut self.global_palette[index.as_usize()]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct Properties {
    pub air_like: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for Properties {
    fn default() -> Self {
        Self { air_like: false }
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Deserialize_repr)]
#[repr(u16)]
pub enum RightAngleRotation {
    #[default]
    Zero = 0,
    Ninety = 90,
    OneEighty = 180,
    TwoSeventy = 270,
}

impl std::ops::Add for RightAngleRotation {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let total_angle = self as u16 + other as u16;
        match total_angle % 360 {
            0 => Self::Zero,
            90 => Self::Ninety,
            180 => Self::OneEighty,
            270 => Self::TwoSeventy,
            _ => unreachable!(),
        }
    }
}

impl std::ops::Sub for RightAngleRotation {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        let total_angle = self as u16 + (360 - other as u16);
        match total_angle % 360 {
            0 => Self::Zero,
            90 => Self::Ninety,
            180 => Self::OneEighty,
            270 => Self::TwoSeventy,
            _ => unreachable!(),
        }
    }
}

impl std::ops::AddAssign for RightAngleRotation {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::SubAssign for RightAngleRotation {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

// Dhall types

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum Registration<'a> {
    #[serde(borrow)]
    Standard(StandardRegistration<'a>),
    #[serde(borrow)]
    FullCustom(FullCustomRegistration<'a>),
    #[serde(borrow)]
    Liquid(LiquidRegistration<'a>),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardRegistration<'a> {
    pub identifier: &'a str,
    #[serde(borrow)]
    pub custom_variants: Option<Vec<CustomProperty<'a>>>,
    #[serde(borrow)]
    pub replacement_variants: Option<Vec<CustomProperty<'a>>>,
    #[serde(borrow)]
    pub default_override: Option<Vec<PropertyValue<'a>>>,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullCustomRegistration<'a> {
    pub identifier: &'a str,
    #[serde(borrow)]
    pub custom_variants: Vec<CustomProperty<'a>>,
    #[serde(borrow)]
    pub skip_properties: Vec<&'a str>,
    #[serde(borrow)]
    pub default_override: Option<Vec<PropertyValue<'a>>>,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiquidRegistration<'a> {
    pub identifier: &'a str,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockstateInfoModifierCase {
    pub modifier: BlockstateInfoModifier,
    pub conditions: Vec<PropertyValueAtoms>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PropertyValueAtoms {
    pub key: Atom,
    pub value: Atom,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomProperty<'a> {
    pub name: &'a str,
    #[serde(borrow)]
    pub prop_type: JsonCustomPropertyType<'a>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
enum JsonCustomPropertyType<'a> {
    Boolean,
    Int {
        start: u32,
        end: u32,
    },
    #[serde(borrow)]
    Enum(Vec<&'a str>),
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PropertyValue<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

pub fn register_blocks_from_json(
    registry: &mut Registry,
    model_cache: &mut ModelCache,
    texture_atlas_builder: &mut texture::AtlasBuilder,
    json_data: &str,
) -> anyhow::Result<()> {
    let registrations: Vec<Registration> = serde_json::from_str(json_data)?;
    for registration in registrations {
        fn convert_custom_property_list<'a>(
            properties: &'a [CustomProperty<'a>],
        ) -> Vec<(&'a str, CustomPropertyType<'a, 'a>)> {
            properties
                .iter()
                .map(move |property| {
                    use JsonCustomPropertyType::*;
                    let prop_type = match &property.prop_type {
                        Boolean => CustomPropertyType::Bool,
                        &Int { start, end } => CustomPropertyType::Int(start..=end),
                        Enum(variants) => CustomPropertyType::Enum(variants.as_slice()),
                    };
                    (property.name, prop_type)
                })
                .collect()
        }
        match registration {
            Registration::Standard(standard_reg) => {
                let identifier = Identifier::parse(standard_reg.identifier)?;
                let custom_variants = standard_reg
                    .custom_variants
                    .as_ref()
                    .map(|variants| convert_custom_property_list(variants.as_slice()));
                let replacement_variants = standard_reg
                    .replacement_variants
                    .as_ref()
                    .map(|variants| convert_custom_property_list(variants.as_slice()));
                let default_override = standard_reg.default_override.as_ref().map(|prop_values| {
                    prop_values
                        .iter()
                        .copied()
                        .map(|PropertyValue { key, value }| (key, value))
                        .collect::<Vec<(&str, &str)>>()
                });
                let extra_info_modifiers = standard_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier,
                            modifier_case
                                .conditions
                                .iter()
                                .map(|kv| (&kv.key, &kv.value))
                                .collect::<Vec<(&Atom, &Atom)>>(),
                        )
                    })
                    .collect::<Vec<(BlockstateInfoModifier, Vec<(&Atom, &Atom)>)>>();
                registry.register(
                    model_cache,
                    texture_atlas_builder,
                    identifier,
                    custom_variants.as_deref(),
                    replacement_variants.as_deref(),
                    default_override.as_deref(),
                    standard_reg.properties,
                    standard_reg.default_extra_info,
                    extra_info_modifiers
                        .iter()
                        .map(|(modifier, conditions)| (*modifier, conditions.as_slice())),
                )?;
            }
            Registration::FullCustom(custom_reg) => {
                let identifier = Identifier::parse(custom_reg.identifier)?;
                let custom_variants = convert_custom_property_list(&custom_reg.custom_variants);
                let default_override = custom_reg.default_override.as_ref().map(|prop_values| {
                    prop_values
                        .iter()
                        .copied()
                        .map(|PropertyValue { key, value }| (key, value))
                        .collect::<Vec<(&str, &str)>>()
                });
                let extra_info_modifiers = custom_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier,
                            modifier_case
                                .conditions
                                .iter()
                                .map(|kv| (&kv.key, &kv.value))
                                .collect::<Vec<(&Atom, &Atom)>>(),
                        )
                    })
                    .collect::<Vec<(BlockstateInfoModifier, Vec<(&Atom, &Atom)>)>>();
                registry.register_full_custom(
                    model_cache,
                    texture_atlas_builder,
                    identifier,
                    &custom_variants,
                    &custom_reg.skip_properties,
                    default_override.as_deref(),
                    custom_reg.properties,
                    custom_reg.default_extra_info,
                    extra_info_modifiers
                        .iter()
                        .map(|(modifier, conditions)| (*modifier, conditions.as_slice())),
                )?;
            }
            Registration::Liquid(liquid_reg) => {
                let identifier = Identifier::parse(liquid_reg.identifier)?;
                let extra_info_modifiers = liquid_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier,
                            modifier_case
                                .conditions
                                .iter()
                                .map(|kv| (&kv.key, &kv.value))
                                .collect::<Vec<(&Atom, &Atom)>>(),
                        )
                    })
                    .collect::<Vec<(BlockstateInfoModifier, Vec<(&Atom, &Atom)>)>>();
                registry.register_liquid(
                    model_cache,
                    texture_atlas_builder,
                    identifier,
                    liquid_reg.properties,
                    liquid_reg.default_extra_info,
                    extra_info_modifiers
                        .iter()
                        .map(|(modifier, conditions)| (*modifier, conditions.as_slice())),
                )?;
            }
        }
    }
    Ok(())
}

pub fn register_vanilla_blocks(
    registry: &mut Registry,
    model_cache: &mut ModelCache,
    texture_atlas_builder: &mut texture::AtlasBuilder,
) -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();

    register_blocks_from_json(
        registry,
        model_cache,
        texture_atlas_builder,
        include_str!("vanilla_blocks_generated.json"),
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
        use ahash::AHashMap;
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
            let identifier = registry
                .get_identifier_from_index(info.block_index)
                .unwrap();
            let block_entry = blocks.entry(identifier.to_string()).or_insert(JsonBlock {
                properties: IndexMap::new(),
                states: Vec::new(),
            });
            let mut properties: IndexMap<_, _> = info
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if let Some(custom_ordering) = custom_ordering_map.get(identifier) {
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
            let default = registry.data[info.block_index]
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
