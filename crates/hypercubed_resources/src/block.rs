pub mod blockstate;
pub mod model;

use crate::{Identifier, RegistryData, RegistryIndex, texture};
use anyhow::{Context, anyhow};
use blockstate::{BlockOpacity, BlockstateInfo, CollisionInfo};
use model::ModelRegistry;
use portable_std::prelude::*;
use portable_std::{Atom, FastHashSet};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[cfg(feature = "std")]
use std_imports::*;
#[cfg(feature = "std")]
mod std_imports {
    pub use super::blockstate::{BlockstateInfoModifier, CustomPropertyType};
    pub use super::model::ModelRegistryBuilder;
}

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub block_registry: Registry,
    pub model_registry: ModelRegistry,
    pub atlas: texture::Atlas,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub data: RegistryData<Info>,
    pub global_palette: Vec<blockstate::Blockstate>,
    pub air_blockstates: FastHashSet<GlobalPaletteIndex>,
    pub light_emitting_blockstates: FastHashSet<GlobalPaletteIndex>,
}

// TODO: Change to `u32`.
// - The memory savings of using a `u16` probably just aren't worth it.
// - Can easily have more than 64K entries.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
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
            air_blockstates: FastHashSet::new(),
            light_emitting_blockstates: FastHashSet::new(),
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

    pub fn light_emitting_blockstates(&self) -> &FastHashSet<GlobalPaletteIndex> {
        &self.light_emitting_blockstates
    }

    /// Panics if an entry is already registered with `identifier`.
    #[expect(clippy::too_many_arguments)]
    #[cfg(feature = "std")]
    pub(super) fn register<'a, I, II>(
        &mut self,
        model_cache: &mut ModelRegistryBuilder,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        custom_variant_properties: Option<&'a [(Atom, CustomPropertyType)]>,
        replacement_variant_properties: Option<&'a [(Atom, CustomPropertyType)]>,
        default_override: Option<&'a [(Atom, Atom)]>,
        properties: Properties,
        default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone
            + core::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
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
            blockstate.extra_info = default_extra_info.clone();
            'case_loop: for (info_modifier, property_set) in extra_info_modifiers_iter.clone() {
                for (k, v) in property_set {
                    let property_value = blockstate.properties.get(k).with_context(|| {
                        format!(
                            "{} \"{}\" {} {:?}",
                            "Extra info property",
                            k,
                            "not found in blockstate properties for",
                            identifier,
                        )
                    })?;
                    if property_value != *v {
                        continue 'case_loop;
                    }
                }
                // If all property conditions in a property modifier match, then apply.
                blockstate.extra_info.merge_modifier(info_modifier);
            }
            // Apply blockstate rotation to custom AABBs
            if let CollisionInfo::Complex(aabbs) = &mut blockstate.extra_info.collision_info {
                for aabb in aabbs.iter_mut() {
                    aabb.apply_blockstate_rotations(
                        blockstate.rough_x_rotation,
                        blockstate.rough_y_rotation,
                    );
                }
            }
        }
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        // Add light-emitting blockstates to global set
        for (i, blockstate) in blockstates.iter_mut().enumerate() {
            if blockstate.extra_info.light_info.emission_level > 0 {
                let palette_idx =
                    GlobalPaletteIndex::try_from(blockstate_id_range.start() + i).unwrap();
                self.light_emitting_blockstates.insert(palette_idx);
            }
        }
        // Calculate default index
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
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
            default_blockstate: GlobalPaletteIndex::try_from(default_index).unwrap(),
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
    #[expect(clippy::too_many_arguments)]
    #[cfg(feature = "std")]
    pub(super) fn register_full_custom<'a, I, II>(
        &mut self,
        model_cache: &mut ModelRegistryBuilder,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        custom_properties: &'a [(Atom, CustomPropertyType)],
        skip_properties: &'a [Atom],
        default_override: Option<&'a [(Atom, Atom)]>,
        properties: Properties,
        default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone
            + core::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
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
            blockstate.extra_info = default_extra_info.clone();
            'case_loop: for (info_modifier, property_set) in extra_info_modifiers_iter.clone() {
                for (k, v) in property_set {
                    if blockstate.properties[k] != **v {
                        continue 'case_loop;
                    }
                }
                // If all property conditions in a property modifier match, then apply.
                blockstate.extra_info.merge_modifier(info_modifier);
            }
            // Apply blockstate rotation to custom AABBs
            if let CollisionInfo::Complex(aabbs) = &mut blockstate.extra_info.collision_info {
                for aabb in aabbs {
                    aabb.apply_blockstate_rotations(
                        blockstate.rough_x_rotation,
                        blockstate.rough_y_rotation,
                    );
                }
            }
        }
        let blockstate_id_range =
            self.global_palette.len()..=self.global_palette.len() + blockstates.len() - 1;
        // Add light-emitting blockstates to global set
        for (i, blockstate) in blockstates.iter_mut().enumerate() {
            if blockstate.extra_info.light_info.emission_level > 0 {
                let palette_idx =
                    GlobalPaletteIndex::try_from(blockstate_id_range.start() + i).unwrap();
                self.light_emitting_blockstates.insert(palette_idx);
            }
        }
        // Calculate default index
        let default_index = match default_override {
            Some(default_override) => {
                let override_map = default_override.iter().cloned().collect();
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

    #[cfg(feature = "std")]
    pub(super) fn register_liquid<'a, I, II>(
        &mut self,
        model_cache: &mut ModelRegistryBuilder,
        texture_atlas: &mut texture::AtlasBuilder,
        identifier: Identifier,
        properties: Properties,
        mut default_extra_info: BlockstateInfo,
        extra_info_modifiers: II,
    ) -> anyhow::Result<RegistryIndex>
    where
        II: IntoIterator<IntoIter = I>,
        I: Clone
            + core::iter::Iterator<Item = (BlockstateInfoModifier, &'a [(&'a Atom, &'a Atom)])>,
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
            blockstate.extra_info = default_extra_info.clone();
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
        // Add light-emitting blockstates to global set
        for (i, blockstate) in blockstates.iter_mut().enumerate() {
            if blockstate.extra_info.light_info.emission_level > 0 {
                let palette_idx =
                    GlobalPaletteIndex::try_from(blockstate_id_range.start() + i).unwrap();
                self.light_emitting_blockstates.insert(palette_idx);
            }
        }
        // Calculate default index
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

impl core::ops::Index<RegistryIndex> for Registry {
    type Output = Info;

    fn index(&self, index: RegistryIndex) -> &Self::Output {
        &self.data[index]
    }
}

impl core::ops::IndexMut<RegistryIndex> for Registry {
    fn index_mut(&mut self, index: RegistryIndex) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl core::ops::Index<GlobalPaletteIndex> for Registry {
    type Output = blockstate::Blockstate;

    fn index(&self, index: GlobalPaletteIndex) -> &blockstate::Blockstate {
        &self.global_palette[index.as_usize()]
    }
}

impl core::ops::IndexMut<GlobalPaletteIndex> for Registry {
    fn index_mut(&mut self, index: GlobalPaletteIndex) -> &mut Self::Output {
        &mut self.global_palette[index.as_usize()]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    pub default_blockstate: GlobalPaletteIndex,
    pub properties: Properties,
    #[cfg(debug_assertions)]
    pub blockstate_id_range: core::ops::RangeInclusive<usize>,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Properties {
    pub air_like: bool,
}

#[expect(clippy::derivable_impls)]
impl Default for Properties {
    fn default() -> Self {
        Self { air_like: false }
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u16)]
pub enum RightAngleRotation {
    #[default]
    Zero = 0,
    Ninety = 90,
    OneEighty = 180,
    TwoSeventy = 270,
}

impl core::ops::Add for RightAngleRotation {
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

impl core::ops::Sub for RightAngleRotation {
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

impl core::ops::AddAssign for RightAngleRotation {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl core::ops::SubAssign for RightAngleRotation {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Registration {
    Standard(StandardRegistration),
    FullCustom(FullCustomRegistration),
    Liquid(LiquidRegistration),
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StandardRegistration {
    pub identifier: Atom,
    pub custom_variants: Option<Vec<CustomProperty>>,
    pub replacement_variants: Option<Vec<CustomProperty>>,
    pub default_override: Option<Vec<PropertyValue>>,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

impl StandardRegistration {
    pub fn new(identifier: impl Into<Atom>) -> Self {
        Self {
            identifier: identifier.into(),
            custom_variants: None,
            replacement_variants: None,
            default_override: None,
            properties: Properties::default(),
            default_extra_info: BlockstateInfo::default(),
            extra_info_modifiers: Vec::new(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FullCustomRegistration {
    pub identifier: Atom,
    pub custom_variants: Vec<CustomProperty>,
    pub skip_properties: Vec<Atom>,
    pub default_override: Option<Vec<PropertyValue>>,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

impl FullCustomRegistration {
    pub fn new(identifier: impl Into<Atom>) -> Self {
        Self {
            identifier: identifier.into(),
            custom_variants: Vec::new(),
            skip_properties: Vec::new(),
            default_override: None,
            properties: Properties::default(),
            default_extra_info: BlockstateInfo::default(),
            extra_info_modifiers: Vec::new(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidRegistration {
    pub identifier: Atom,
    pub properties: Properties,
    pub default_extra_info: BlockstateInfo,
    pub extra_info_modifiers: Vec<BlockstateInfoModifierCase>,
}

impl LiquidRegistration {
    pub fn new(identifier: impl Into<Atom>) -> Self {
        Self {
            identifier: identifier.into(),
            properties: Properties::default(),
            default_extra_info: BlockstateInfo::default(),
            extra_info_modifiers: Vec::new(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockstateInfoModifierCase {
    pub modifier: BlockstateInfoModifier,
    pub conditions: Vec<PropertyValue>,
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropertyValue {
    pub key: Atom,
    pub value: Atom,
}

impl From<(&str, &str)> for PropertyValue {
    fn from((key, value): (&str, &str)) -> Self {
        Self::new(key, value)
    }
}

impl PropertyValue {
    pub fn new(key: impl Into<Atom>, value: impl Into<Atom>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomProperty {
    pub name: Atom,
    pub prop_type: SerialisedCustomPropertyType,
}

impl CustomProperty {
    pub fn boolean(name: impl Into<Atom>) -> Self {
        Self {
            name: name.into(),
            prop_type: SerialisedCustomPropertyType::Boolean,
        }
    }

    pub fn int(name: impl Into<Atom>, range: core::ops::RangeInclusive<u32>) -> Self {
        Self {
            name: name.into(),
            prop_type: SerialisedCustomPropertyType::Int {
                start: *range.start(),
                end: *range.end(),
            },
        }
    }

    pub fn enum_variants(
        name: impl Into<Atom>,
        variants: impl IntoIterator<Item = impl Into<Atom>>,
    ) -> Self {
        Self {
            name: name.into(),
            prop_type: SerialisedCustomPropertyType::Enum(
                variants.into_iter().map(Into::into).collect(),
            ),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SerialisedCustomPropertyType {
    Boolean,
    Int { start: u32, end: u32 },
    Enum(Vec<Atom>),
}

#[cfg(feature = "std")]
pub fn register_blocks_from_list(
    registry: &mut Registry,
    model_cache: &mut ModelRegistryBuilder,
    texture_atlas_builder: &mut texture::AtlasBuilder,
    registrations: impl IntoIterator<Item = Registration>,
) -> anyhow::Result<()> {
    for registration in registrations.into_iter() {
        fn convert_custom_property_list<'a>(
            properties: &'a [CustomProperty],
        ) -> Vec<(Atom, CustomPropertyType<'a>)> {
            properties
                .iter()
                .map(move |property| {
                    use SerialisedCustomPropertyType::*;
                    let prop_type = match &property.prop_type {
                        Boolean => CustomPropertyType::Bool,
                        Int { start, end } => CustomPropertyType::Int(*start..=*end),
                        Enum(variants) => CustomPropertyType::Enum(variants.as_slice()),
                    };
                    (property.name.clone(), prop_type)
                })
                .collect()
        }
        match registration {
            Registration::Standard(standard_reg) => {
                let identifier = Identifier::parse(&standard_reg.identifier)?;
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
                        .map(|PropertyValue { key, value }| (key.clone(), value.clone()))
                        .collect::<Vec<(Atom, Atom)>>()
                });
                let extra_info_modifiers = standard_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier.clone(),
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
                        .map(|(modifier, conditions)| (modifier.clone(), conditions.as_slice())),
                )?;
            }
            Registration::FullCustom(custom_reg) => {
                let identifier = Identifier::parse(&custom_reg.identifier)?;
                let custom_variants = convert_custom_property_list(&custom_reg.custom_variants);
                let default_override = custom_reg.default_override.as_ref().map(|prop_values| {
                    prop_values
                        .iter()
                        .cloned()
                        .map(|PropertyValue { key, value }| (key, value))
                        .collect::<Vec<(Atom, Atom)>>()
                });
                let extra_info_modifiers = custom_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier.clone(),
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
                        .map(|(modifier, conditions)| (modifier.clone(), conditions.as_slice())),
                )?;
            }
            Registration::Liquid(liquid_reg) => {
                let identifier = Identifier::parse(&liquid_reg.identifier)?;
                let extra_info_modifiers = liquid_reg
                    .extra_info_modifiers
                    .iter()
                    .map(|modifier_case| {
                        (
                            modifier_case.modifier.clone(),
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
                        .map(|(modifier, conditions)| (modifier.clone(), conditions.as_slice())),
                )?;
            }
        }
    }
    Ok(())
}
