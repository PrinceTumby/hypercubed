use super::model::{CombinedModelPart, ModelCache, ModelType};
use super::RightAngleRotation;
use super::{texture, Identifier};
use crate::resource::manager::{get_resource_file, ResourceType};
use crate::resource::RegistryIndex;
use ahash::{AHashMap, AHashSet};
use anyhow::{anyhow, bail, ensure, Context};
use indexmap::IndexMap;
use serde::Deserialize;
use std::borrow::Cow;
use std::fmt::Write;
use std::rc::Rc;
use string_cache::DefaultAtom as Atom;

pub fn load_blockstates(
    block_index: RegistryIndex,
    identifier: &Identifier,
    custom_properties: Option<&[(&str, CustomPropertyType)]>,
    replacement_properties: Option<&[(&str, CustomPropertyType)]>,
    model_cache: &mut ModelCache,
    texture_atlas: &mut texture::AtlasBuilder,
) -> anyhow::Result<Vec<Blockstate>> {
    let blockstate_json_bytes = get_resource_file(&ResourceType::Blockstate, identifier)
        .with_context(|| format!("Failed to read raw blockstate JSON data for {identifier:?}"))?;
    let file: File = serde_json::from_slice(&blockstate_json_bytes)
        .with_context(|| format!("Failed to parse blockstate JSON data for {identifier:?}"))?;
    match file {
        File::Variants(variants) => load_blockstate_variants(
            block_index,
            identifier,
            variants,
            custom_properties.unwrap_or(&[]),
            replacement_properties,
            model_cache,
            texture_atlas,
        )
        .context("Failed to load blockstate variants"),
        File::Multipart(cases) => {
            assert!(
                replacement_properties.is_none(),
                "replacement properties not valid for multipart blockstates"
            );
            load_blockstate_multipart_cases(
                block_index,
                identifier,
                cases,
                custom_properties
                    .expect("properties must be specified manually for multipart blockstates"),
                model_cache,
                texture_atlas,
            )
            .context("Failed to load blockstate variants")
        }
    }
}

fn load_blockstate_variants(
    block_index: RegistryIndex,
    identifier: &Identifier,
    variants: IndexMap<String, Variant>,
    custom_properties: &[(&str, CustomPropertyType)],
    replacement_properties: Option<&[(&str, CustomPropertyType)]>,
    model_cache: &mut ModelCache,
    texture_atlas: &mut texture::AtlasBuilder,
) -> anyhow::Result<Vec<Blockstate>> {
    // Check condition sets
    for condition_set in variants.keys() {
        if !is_valid_condition_set(condition_set) {
            bail!("Invalid condition set {condition_set}");
        }
    }
    // Check none of the custom variants are also in the blockstate file
    for (custom_property_name, _) in custom_properties {
        if let Some(overlapping_variant) = variants
            .keys()
            .flat_map(|condition_set| {
                condition_set
                    .split(',')
                    .filter(|condition| condition != &"")
                    .map(|condition| condition.split_once('=').unwrap().0)
            })
            .find(|variant_name| variant_name == custom_property_name)
        {
            bail!(
                "Custom variant {:?} also found in blockstate file",
                overlapping_variant,
            );
        }
    }
    let mut custom_property_iters: Vec<_> = custom_properties
        .iter()
        .map(|(name, ty)| (*name, ty.iter()))
        .collect();
    let mut current_custom_property_states: Vec<_> = custom_property_iters
        .iter_mut()
        .map(|(_name, iter)| iter.next().0)
        .collect();
    let mut is_final_custom_state = false;
    if let Some(replacement_properties) = replacement_properties {
        // Check every replacement property is replacing something in the blockstate file.
        // Doesn't check if they're the same type.
        for (property_name, _) in replacement_properties {
            if variants
                .keys()
                .flat_map(|condition_set| {
                    condition_set
                        .split(',')
                        .filter(|condition| condition != &"")
                        .map(|condition| condition.split_once('=').unwrap().0)
                })
                .find(|variant_name| variant_name == property_name)
                .is_none()
            {
                bail!(
                    "Replacement property {:?} not found in blockstate file",
                    replacement_properties
                );
            }
        }
        let mut property_iters: Vec<_> = replacement_properties
            .iter()
            .map(|(name, ty)| (*name, ty.iter()))
            .collect();
        let mut current_property_states: Vec<_> = property_iters
            .iter_mut()
            .map(|(_name, iter)| iter.next().0)
            .collect();
        let mut is_final_state = false;
        let mut blockstates = Vec::new();
        while !is_final_state {
            // Get current state
            let state: Vec<_> = property_iters
                .iter()
                .zip(current_property_states.iter())
                .map(|((name, _iter), value)| (*name, value.clone()))
                .collect();
            // Generate next state
            for (i, ((_name, property_iter), state)) in property_iters
                .iter_mut()
                .zip(current_property_states.iter_mut())
                .enumerate()
                .rev()
            {
                let (new_state, iter_reset) = property_iter.next();
                *state = new_state;
                if !iter_reset {
                    break;
                } else if i == 0 {
                    is_final_state = true;
                }
            }
            // Generate condition set string (for finding entry), and map (stored as extra info)
            let mut condition_set = String::new();
            let mut condition_map = AHashMap::new();
            for (i, (name, value)) in state.into_iter().enumerate() {
                if i > 0 {
                    write!(&mut condition_set, ",").unwrap();
                }
                write!(&mut condition_set, "{name}={value}").unwrap();
                condition_map.insert(Atom::from(name), Atom::from(value));
            }
            let variant = &variants[&condition_set];
            match variant {
                Variant::Single(model_info) => {
                    let model_location =
                        Identifier::parse(&model_info.model).with_context(|| {
                            format!("Failed to parse {:?} as identifier", &model_info.model)
                        })?;
                    let model = model_cache
                        .load_model(&model_location, texture_atlas)
                        .with_context(|| format!("Failed to load model {model_location:?}"))?;
                    while !is_final_custom_state {
                        if custom_properties.is_empty() {
                            is_final_custom_state = true;
                        }
                        // Get current custom state
                        let custom_state: Vec<_> = custom_property_iters
                            .iter()
                            .zip(current_custom_property_states.iter())
                            .map(|((name, _iter), value)| (*name, value.clone()))
                            .collect();
                        // Generate next custom state
                        for (i, ((_name, property_iter), state)) in custom_property_iters
                            .iter_mut()
                            .zip(current_custom_property_states.iter_mut())
                            .enumerate()
                            .rev()
                        {
                            let (new_state, iter_reset) = property_iter.next();
                            *state = new_state;
                            if !iter_reset {
                                break;
                            } else if i == 0 {
                                is_final_custom_state = true;
                            }
                        }
                        let mut condition_map = condition_map.clone();
                        for (name, value) in custom_state.into_iter() {
                            condition_map.insert(Atom::from(name), Atom::from(value));
                        }
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::Single(Model {
                                model: model.clone(),
                                x_rotation: model_info.x_rotation,
                                y_rotation: model_info.y_rotation,
                            }),
                        });
                    }
                    is_final_custom_state = false;
                }
                Variant::List(models) => {
                    let mut models = models
                        .into_iter()
                        .map(|model_info| {
                            let model_location = Identifier::parse(&model_info.model)?;
                            let model = model_cache.load_model(&model_location, texture_atlas)?;
                            Ok(WeightedModel {
                                model,
                                x_rotation: model_info.x_rotation,
                                y_rotation: model_info.y_rotation,
                                weight: model_info.weight,
                            })
                        })
                        .collect::<anyhow::Result<Box<[WeightedModel]>>>()?;
                    // Rescale weights so all sum to 1.0
                    {
                        let total_weight: f32 = models.iter().map(|variant| variant.weight).sum();
                        for model in models.iter_mut() {
                            model.weight /= total_weight;
                        }
                    }
                    while !is_final_custom_state {
                        if custom_properties.is_empty() {
                            is_final_custom_state = true;
                        }
                        // Get current custom state
                        let custom_state: Vec<_> = custom_property_iters
                            .iter()
                            .zip(current_custom_property_states.iter())
                            .map(|((name, _iter), value)| (*name, value.clone()))
                            .collect();
                        // Generate next custom state
                        for (i, ((_name, property_iter), state)) in custom_property_iters
                            .iter_mut()
                            .zip(current_custom_property_states.iter_mut())
                            .enumerate()
                            .rev()
                        {
                            let (new_state, iter_reset) = property_iter.next();
                            *state = new_state;
                            if !iter_reset {
                                break;
                            } else if i == 0 {
                                is_final_custom_state = true;
                            }
                        }
                        let mut condition_map = condition_map.clone();
                        for (name, value) in custom_state.into_iter() {
                            condition_map.insert(Atom::from(name), Atom::from(value));
                        }
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::RandomChoice(models.clone()),
                        });
                    }
                    is_final_custom_state = false;
                }
            }
        }
        Ok(blockstates)
    } else {
        let mut blockstates = Vec::new();
        for (condition_set, variant) in variants {
            if !is_valid_condition_set(&condition_set) {
                bail!("Invalid condition set {condition_set}");
            }
            let condition_map: AHashMap<Atom, Atom> = condition_set
                .split(',')
                .filter(|condition| condition != &"")
                .map(|condition| {
                    let (condition, value) = condition.split_once('=').unwrap();
                    (Atom::from(condition), Atom::from(value))
                })
                .collect();
            match variant {
                Variant::Single(model_info) => {
                    let model_location =
                        Identifier::parse(&model_info.model).with_context(|| {
                            format!("Failed to parse {:?} as identifier", &model_info.model)
                        })?;
                    let model = model_cache
                        .load_model(&model_location, texture_atlas)
                        .with_context(|| format!("Failed to load model {model_location:?}"))?;
                    while !is_final_custom_state {
                        if custom_properties.is_empty() {
                            is_final_custom_state = true;
                        }
                        // Get current custom state
                        let custom_state: Vec<_> = custom_property_iters
                            .iter()
                            .zip(current_custom_property_states.iter())
                            .map(|((name, _iter), value)| (*name, value.clone()))
                            .collect();
                        // Generate next custom state
                        for (i, ((_name, property_iter), state)) in custom_property_iters
                            .iter_mut()
                            .zip(current_custom_property_states.iter_mut())
                            .enumerate()
                            .rev()
                        {
                            let (new_state, iter_reset) = property_iter.next();
                            *state = new_state;
                            if !iter_reset {
                                break;
                            } else if i == 0 {
                                is_final_custom_state = true;
                            }
                        }
                        let mut condition_map = condition_map.clone();
                        for (name, value) in custom_state.into_iter() {
                            condition_map.insert(Atom::from(name), Atom::from(value));
                        }
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::Single(Model {
                                model: model.clone(),
                                x_rotation: model_info.x_rotation,
                                y_rotation: model_info.y_rotation,
                            }),
                        });
                    }
                    is_final_custom_state = false;
                }
                Variant::List(models) => {
                    let mut models = models
                        .into_iter()
                        .map(|model_info| {
                            let model_location = Identifier::parse(&model_info.model)?;
                            let model = model_cache.load_model(&model_location, texture_atlas)?;
                            Ok(WeightedModel {
                                model,
                                x_rotation: model_info.x_rotation,
                                y_rotation: model_info.y_rotation,
                                weight: model_info.weight,
                            })
                        })
                        .collect::<anyhow::Result<Box<[WeightedModel]>>>()?;
                    // Rescale weights so all sum to 1.0
                    {
                        let total_weight: f32 = models.iter().map(|variant| variant.weight).sum();
                        for model in models.iter_mut() {
                            model.weight /= total_weight;
                        }
                    }
                    while !is_final_custom_state {
                        if custom_properties.is_empty() {
                            is_final_custom_state = true;
                        }
                        // Get current custom state
                        let custom_state: Vec<_> = custom_property_iters
                            .iter()
                            .zip(current_custom_property_states.iter())
                            .map(|((name, _iter), value)| (*name, value.clone()))
                            .collect();
                        // Generate next custom state
                        for (i, ((_name, property_iter), state)) in custom_property_iters
                            .iter_mut()
                            .zip(current_custom_property_states.iter_mut())
                            .enumerate()
                            .rev()
                        {
                            let (new_state, iter_reset) = property_iter.next();
                            *state = new_state;
                            if !iter_reset {
                                break;
                            } else if i == 0 {
                                is_final_custom_state = true;
                            }
                        }
                        let mut condition_map = condition_map.clone();
                        for (name, value) in custom_state.into_iter() {
                            condition_map.insert(Atom::from(name), Atom::from(value));
                        }
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::RandomChoice(models.clone()),
                        });
                    }
                    is_final_custom_state = false;
                }
            }
        }
        Ok(blockstates)
    }
}

#[tracing::instrument(skip(cases, properties, model_cache, texture_atlas))]
fn load_blockstate_multipart_cases(
    block_index: RegistryIndex,
    identifier: &Identifier,
    mut cases: Vec<MultipartCase>,
    properties: &[(&str, CustomPropertyType)],
    model_cache: &mut ModelCache,
    texture_atlas: &mut texture::AtlasBuilder,
) -> anyhow::Result<Vec<Blockstate>> {
    // Rescale weighted cases, so for each case, all weights sum to 1.0
    for case in &mut cases {
        if let Variant::List(ref mut variants) = case.apply_variant {
            let total_weight: f32 = variants.iter().map(|variant| variant.weight).sum();
            for model in variants {
                model.weight /= total_weight;
            }
        }
    }
    let mut property_iters: Vec<_> = properties
        .iter()
        .map(|(name, ty)| (*name, ty.iter()))
        .collect();
    let mut current_property_states: Vec<_> = property_iters
        .iter_mut()
        .map(|(_name, iter)| iter.next().0)
        .collect();
    let mut is_final_state = false;
    let mut blockstates = Vec::new();
    while !is_final_state {
        // Get current state
        let state: AHashMap<_, _> = property_iters
            .iter()
            .zip(current_property_states.iter())
            .map(|((name, _iter), value)| (*name, value.clone()))
            .collect();
        // Generate next state
        for (i, ((_name, property_iter), state)) in property_iters
            .iter_mut()
            .zip(current_property_states.iter_mut())
            .enumerate()
            .rev()
        {
            let (new_state, iter_reset) = property_iter.next();
            *state = new_state;
            if !iter_reset {
                break;
            } else if i == 0 {
                is_final_state = true;
            }
        }
        let condition_map: AHashMap<Atom, Atom> = state
            .iter()
            .map(|(&k, v)| (Atom::from(k), Atom::from(v.as_ref())))
            .collect();
        // Some blockstate files use a multipart with only one variant (useful for generating
        // models with randomised parts)
        if properties.len() == 0 {
            is_final_state = true;
        }
        log::debug!("Generating model for state {state:?}");
        #[derive(Clone, Debug)]
        struct WeightedModelPartGroup {
            pub parts: Vec<CombinedModelPart>,
            pub weight: f32,
        }
        let mut model_part_groups = vec![WeightedModelPartGroup {
            parts: Vec::new(),
            weight: 1.0,
        }];
        for case in &cases {
            let condition_satisfied = match &case.condition {
                None => true,
                Some(condition_container) => match condition_container {
                    MultipartConditionContainer::Single(condition_group) => {
                        is_multipart_condition_group_satisfied(&state, condition_group)
                    }
                    MultipartConditionContainer::Combination(combination) => match combination {
                        MultipartConditionCombination::Union(conditions) => {
                            conditions.iter().any(|condition_group| {
                                is_multipart_condition_group_satisfied(&state, condition_group)
                            })
                        }
                        MultipartConditionCombination::Intersection(conditions) => {
                            conditions.iter().all(|condition_group| {
                                is_multipart_condition_group_satisfied(&state, condition_group)
                            })
                        }
                    },
                },
            };
            if condition_satisfied {
                match &case.apply_variant {
                    Variant::Single(variant) => {
                        let model_part = CombinedModelPart {
                            location: Identifier::parse(&variant.model).with_context(|| {
                                format!("Failed to parse {:?} as identifier", &variant.model)
                            })?,
                            x_rotation: variant.x_rotation,
                            y_rotation: variant.y_rotation,
                            uv_lock: variant.uv_lock,
                        };
                        for group in &mut model_part_groups {
                            group.parts.push(model_part.clone());
                        }
                    }
                    Variant::List(variants) => {
                        let mut new_model_part_groups = Vec::new();
                        for variant in variants {
                            let variant_part = CombinedModelPart {
                                location: Identifier::parse(&variant.model).with_context(|| {
                                    format!("Failed to parse {:?} as identifier", &variant.model)
                                })?,
                                x_rotation: variant.x_rotation,
                                y_rotation: variant.y_rotation,
                                uv_lock: variant.uv_lock,
                            };
                            let mut combined_group_set = model_part_groups.clone();
                            for group in &mut combined_group_set {
                                group.parts.push(variant_part.clone());
                                group.weight *= variant.weight
                            }
                            new_model_part_groups.extend(combined_group_set.drain(..));
                        }
                        model_part_groups = new_model_part_groups;
                    }
                }
            }
        }
        if model_part_groups.len() == 1 && model_part_groups[0].parts.is_empty() {
            log::debug!("^ no parts");
        }
        // Generate models for current state
        if let &[ref model_parts] = &model_part_groups[..] {
            // Only one possible model
            let model = model_cache
                .load_combined_model(&model_parts.parts, texture_atlas)
                .with_context(|| format!("Error combining model list {:?}", &model_parts.parts))?;
            blockstates.push(Blockstate {
                block_index,
                properties: condition_map,
                model_data: ModelData::Single(Model {
                    model,
                    x_rotation: RightAngleRotation::Zero,
                    y_rotation: RightAngleRotation::Zero,
                }),
            });
        } else {
            // Multiple possible models, generate one for each possibility
            let models = model_part_groups
                .into_iter()
                .map(|group| {
                    let model = model_cache
                        .load_combined_model(&group.parts, texture_atlas)
                        .with_context(|| {
                            format!("Error combining model list {:?}", &group.parts)
                        })?;
                    Ok(WeightedModel {
                        model,
                        x_rotation: RightAngleRotation::Zero,
                        y_rotation: RightAngleRotation::Zero,
                        weight: group.weight,
                    })
                })
                .collect::<anyhow::Result<_>>()?;
            blockstates.push(Blockstate {
                block_index,
                properties: condition_map,
                model_data: ModelData::RandomChoice(models),
            })
        }
    }
    Ok(blockstates)
}

fn is_multipart_condition_group_satisfied(
    state: &AHashMap<&str, Cow<'_, str>>,
    condition_group: &MultipartConditionGroup,
) -> bool {
    for condition in &condition_group.0 {
        let mut found_condition = false;
        for allowed_property in &condition.property_set {
            let Some(state_value) = state.get(allowed_property.as_str()) else {
                continue;
            };
            if condition.value_set.contains(state_value.as_ref()) {
                found_condition = true;
                break;
            }
        }
        if !found_condition {
            return false;
        }
    }
    true
}

/// `custom_properties` is each property that defines the blockstates, in order.
/// `skip_properties` is a list of property names from `properties` that do not appear in the
/// blockstates file.
pub fn load_full_custom_blockstates(
    block_index: RegistryIndex,
    identifier: &Identifier,
    properties: &[(&str, CustomPropertyType)],
    skip_properties: &[&str],
    model_cache: &mut ModelCache,
    texture_atlas: &mut texture::AtlasBuilder,
) -> anyhow::Result<Vec<Blockstate>> {
    // Check all skip properties are actually also properties
    for skip_prop in skip_properties {
        assert!(
            properties
                .iter()
                .find(|(prop_name, _)| prop_name == skip_prop)
                .is_some(),
            "Skip property '{skip_prop}' not found in properties"
        );
    }
    let skip_properties: AHashSet<_> = skip_properties
        .into_iter()
        .map(|p| Atom::from(*p))
        .collect();
    let blockstate_json_bytes = get_resource_file(&ResourceType::Blockstate, identifier)
        .with_context(|| format!("Failed to read raw blockstate JSON data for {identifier:?}"))?;
    let file: File = serde_json::from_slice(&blockstate_json_bytes)
        .with_context(|| format!("Failed to parse blockstate JSON data for {identifier:?}"))?;
    match file {
        File::Variants(variants) => {
            // Check condition sets
            for condition_set in variants.keys() {
                if !is_valid_condition_set(condition_set) {
                    bail!("Invalid condition set {condition_set}");
                }
            }
            let mut property_iters: Vec<_> = properties
                .iter()
                .map(|(name, ty)| (Atom::from(*name), ty.iter()))
                .collect();
            let mut current_property_states: Vec<_> = property_iters
                .iter_mut()
                .map(|(_name, iter)| iter.next().0)
                .collect();
            let mut is_final_state = false;
            let mut blockstates = Vec::new();
            while !is_final_state {
                // Get current state
                let state: Vec<_> = property_iters
                    .iter()
                    .zip(current_property_states.iter())
                    .map(|((name, _iter), value)| (name.clone(), value.clone()))
                    .collect();
                // Generate next state
                for (i, ((_name, property_iter), state)) in property_iters
                    .iter_mut()
                    .zip(current_property_states.iter_mut())
                    .enumerate()
                    .rev()
                {
                    let (new_state, iter_reset) = property_iter.next();
                    *state = new_state;
                    if !iter_reset {
                        break;
                    } else if i == 0 {
                        is_final_state = true;
                    }
                }
                // Generate condition set string (for finding entry), and map (stored as extra info)
                let mut condition_strings = Vec::new();
                let mut condition_map = AHashMap::new();
                for (name, value) in state {
                    if !skip_properties.contains(&name) {
                        condition_strings.push(format!("{name}={value}"));
                    }
                    condition_map.insert(Atom::from(name), Atom::from(value));
                }
                condition_strings.sort_unstable();
                let condition_set = condition_strings.join(",");
                if !variants.contains_key(&condition_set) {
                    dbg!(&condition_set);
                }
                let variant = &variants[&condition_set];
                match variant {
                    Variant::Single(model_info) => {
                        let model_location =
                            Identifier::parse(&model_info.model).with_context(|| {
                                format!("Failed to parse {:?} as identifier", &model_info.model)
                            })?;
                        let model = model_cache
                            .load_model(&model_location, texture_atlas)
                            .with_context(|| format!("Failed to load model {model_location:?}"))?;
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::Single(Model {
                                model: model,
                                x_rotation: model_info.x_rotation,
                                y_rotation: model_info.y_rotation,
                            }),
                        });
                    }
                    Variant::List(models) => {
                        let mut models = models
                            .into_iter()
                            .map(|model_info| {
                                let model_location = Identifier::parse(&model_info.model)?;
                                let model =
                                    model_cache.load_model(&model_location, texture_atlas)?;
                                Ok(WeightedModel {
                                    model,
                                    x_rotation: model_info.x_rotation,
                                    y_rotation: model_info.y_rotation,
                                    weight: model_info.weight,
                                })
                            })
                            .collect::<anyhow::Result<Box<[WeightedModel]>>>()?;
                        // Rescale weights so all sum to 1.0
                        {
                            let total_weight: f32 =
                                models.iter().map(|variant| variant.weight).sum();
                            for model in models.iter_mut() {
                                model.weight /= total_weight;
                            }
                        }
                        blockstates.push(Blockstate {
                            block_index,
                            properties: condition_map,
                            model_data: ModelData::RandomChoice(models.clone()),
                        });
                    }
                }
            }
            Ok(blockstates)
        }
        File::Multipart(_cases) => unimplemented!(),
    }
}

pub fn load_liquid_blockstates(
    block_index: RegistryIndex,
    identifier: &Identifier,
    model_cache: &mut ModelCache,
    texture_atlas: &mut texture::AtlasBuilder,
) -> anyhow::Result<Vec<Blockstate>> {
    let blockstate_json_bytes = get_resource_file(&ResourceType::Blockstate, identifier)
        .with_context(|| format!("Failed to read raw blockstate JSON data for {identifier:?}"))?;
    let file: File = serde_json::from_slice(&blockstate_json_bytes)
        .with_context(|| format!("Failed to parse blockstate JSON data for {identifier:?}"))?;
    // println!("{file:?}");
    let File::Variants(mut variants) = file else {
        return Err(anyhow!(
            "Invalid liquid blockstates {file:?} for {identifier:?}"
        ));
    };
    ensure!(
        variants.len() == 1 && variants.contains_key(""),
        "Liquid {identifier:?} must only contain an empty variant"
    );
    let Variant::Single(variant_model) = variants.swap_remove("").unwrap() else {
        return Err(anyhow!(
            "Invalid liquid variant {variants:?} for {identifier:?}"
        ));
    };
    ensure!(
        [variant_model.x_rotation, variant_model.y_rotation] == [RightAngleRotation::Zero; 2],
        "Liquid {identifier:?} blockstate variant should not include rotations",
    );
    ensure!(
        !variant_model.uv_lock,
        "Liquid {identifier:?} blockstate should not define UV Lock"
    );
    let model_location = Identifier::parse(&variant_model.model)
        .with_context(|| format!("Failed to parse model identifier {:?}", variant_model.model))?;
    let model = model_cache
        .load_liquid(&model_location, texture_atlas)
        .with_context(|| format!("Failed to load liquid model for {model_location:?}"))?;
    let mut blockstates = Vec::new();
    // Liquids have blockstates for levels 0-15
    for i in 0..16 {
        blockstates.push(Blockstate {
            block_index,
            properties: [(Atom::from("level"), Atom::from(format!("{i}")))]
                .into_iter()
                .collect(),
            model_data: ModelData::Single(Model {
                model: model.clone(),
                x_rotation: RightAngleRotation::Zero,
                y_rotation: RightAngleRotation::Zero,
            }),
        });
    }
    Ok(blockstates)
}

fn is_valid_condition_set(set: &str) -> bool {
    if set == "" {
        return true;
    }
    #[derive(Clone, Copy)]
    enum State {
        VariableStart,
        Variable,
        ValueStart,
        Value,
    }
    let mut state = State::VariableStart;
    for c in set.chars() {
        match (state, c) {
            (State::VariableStart, 'a'..='z' | '0'..='9' | '_') => state = State::Variable,
            (State::Variable, 'a'..='z' | '0'..='9' | '_') => {}
            (State::Variable, '=') => state = State::ValueStart,
            (State::ValueStart, 'a'..='z' | '0'..='9' | '_') => state = State::Value,
            (State::Value, 'a'..='z' | '0'..='9' | '_') => {}
            (State::Value, ',') => state = State::VariableStart,
            _ => return false,
        }
    }
    matches!(state, State::Value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomPropertyType<'a, 'b> {
    Bool,
    Int(std::ops::RangeInclusive<u32>),
    Enum(&'a [&'b str]),
}

impl CustomPropertyType<'_, '_> {
    pub fn iter(&self) -> CustomPropertyIterator {
        let type_state = match self {
            Self::Bool => CustomPropertyIteratorType::Bool {
                current_state: true,
            },
            Self::Int(range) => CustomPropertyIteratorType::Int {
                range: range.clone(),
                current_num: *range.start(),
            },
            Self::Enum(kinds) => CustomPropertyIteratorType::Enum {
                kinds,
                current_index: 0,
            },
        };
        CustomPropertyIterator {
            type_state,
            just_reset: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CustomPropertyIterator<'a, 'b> {
    type_state: CustomPropertyIteratorType<'a, 'b>,
    just_reset: bool,
}

#[derive(Clone, Debug)]
pub enum CustomPropertyIteratorType<'a, 'b> {
    Bool {
        current_state: bool,
    },
    Int {
        range: std::ops::RangeInclusive<u32>,
        current_num: u32,
    },
    Enum {
        kinds: &'a [&'b str],
        current_index: usize,
    },
}

impl<'b> CustomPropertyIterator<'_, 'b> {
    /// Returns the next value, along with whether the value has just reset
    pub fn next(&mut self) -> (Cow<'b, str>, bool) {
        let (state, is_last_state) = match &mut self.type_state {
            CustomPropertyIteratorType::Bool {
                current_state: state,
            } => {
                let current_state = *state;
                *state = !current_state;
                (
                    Cow::Borrowed(if current_state { "true" } else { "false" }),
                    !current_state,
                )
            }
            CustomPropertyIteratorType::Int { range, current_num } => {
                if range.contains(&(*current_num + 1)) {
                    let num = *current_num;
                    *current_num += 1;
                    (Cow::Owned(format!("{num}")), false)
                } else {
                    (
                        Cow::Owned(format!(
                            "{}",
                            std::mem::replace(current_num, *range.start())
                        )),
                        true,
                    )
                }
            }
            CustomPropertyIteratorType::Enum {
                kinds,
                current_index,
            } => {
                if *current_index + 1 < kinds.len() {
                    let index = *current_index;
                    *current_index += 1;
                    (Cow::Borrowed(kinds[index]), false)
                } else {
                    (
                        Cow::Borrowed(kinds[std::mem::replace(current_index, 0)]),
                        true,
                    )
                }
            }
        };
        (
            state,
            std::mem::replace(&mut self.just_reset, is_last_state),
        )
    }
}

#[derive(Clone, Debug)]
pub struct Blockstate {
    pub block_index: RegistryIndex,
    pub properties: AHashMap<Atom, Atom>,
    pub model_data: ModelData,
}

#[derive(Clone, Debug)]
pub enum ModelData {
    Single(Model),
    RandomChoice(Box<[WeightedModel]>),
}

#[derive(Clone, Debug)]
pub struct Model {
    pub model: Rc<ModelType>,
    pub x_rotation: RightAngleRotation,
    pub y_rotation: RightAngleRotation,
}

#[derive(Clone, Debug)]
pub struct WeightedModel {
    pub model: Rc<ModelType>,
    pub x_rotation: RightAngleRotation,
    pub y_rotation: RightAngleRotation,
    pub weight: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum File {
    Variants(IndexMap<String, Variant>),
    Multipart(Vec<MultipartCase>),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Variant {
    Single(VariantModel),
    List(Vec<WeightedVariantModel>),
}

#[derive(Clone, Debug, Deserialize)]
pub struct VariantModel {
    pub model: String,
    #[serde(default, rename = "x")]
    pub x_rotation: RightAngleRotation,
    #[serde(default, rename = "y")]
    pub y_rotation: RightAngleRotation,
    #[serde(default, rename = "uvlock")]
    pub uv_lock: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WeightedVariantModel {
    pub model: String,
    #[serde(default, rename = "x")]
    pub x_rotation: RightAngleRotation,
    #[serde(default, rename = "y")]
    pub y_rotation: RightAngleRotation,
    #[serde(default, rename = "uvlock")]
    pub uv_lock: bool,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

#[derive(Clone, Debug, Deserialize)]
struct MultipartCase {
    #[serde(rename = "when")]
    pub condition: Option<MultipartConditionContainer>,
    #[serde(rename = "apply")]
    pub apply_variant: Variant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum MultipartConditionContainer {
    Single(MultipartConditionGroup),
    Combination(MultipartConditionCombination),
}

#[derive(Clone, Debug, Deserialize)]
enum MultipartConditionCombination {
    #[serde(rename = "OR")]
    Union(Vec<MultipartConditionGroup>),
    #[serde(rename = "AND")]
    Intersection(Vec<MultipartConditionGroup>),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "AHashMap<String, String>")]
pub struct MultipartConditionGroup(Vec<MultipartCondition>);

#[derive(Clone, Debug)]
struct MultipartCondition {
    pub property_set: AHashSet<String>,
    pub value_set: AHashSet<String>,
}

impl TryFrom<AHashMap<String, String>> for MultipartConditionGroup {
    type Error = String;

    fn try_from(conditions: AHashMap<String, String>) -> Result<Self, Self::Error> {
        if conditions.is_empty() {
            return Err("MultipartCondition requires at least one condition".to_string());
        }
        let converted_conditions: Result<Vec<MultipartCondition>, _> = conditions
            .into_iter()
            .map(|(property_set_string, value_set_string)| {
                if !is_valid_property_or_value_set(&property_set_string) {
                    return Err(format!("invalid property set {property_set_string:?}"));
                }
                if !is_valid_property_or_value_set(&value_set_string) {
                    return Err(format!("invalid value set {value_set_string:?}"));
                }
                let property_set = property_set_string.split('|').map(String::from).collect();
                let value_set = value_set_string.split('|').map(String::from).collect();
                Ok(MultipartCondition {
                    property_set,
                    value_set,
                })
            })
            .collect();
        converted_conditions.map(MultipartConditionGroup)
    }
}

fn is_valid_property_or_value_set(set: &str) -> bool {
    #[derive(Clone, Copy)]
    enum State {
        PropertyStart,
        Property,
    }
    let mut state = State::PropertyStart;
    for c in set.chars() {
        match (state, c) {
            (State::PropertyStart, 'a'..='z' | '0'..='9' | '_') => state = State::Property,
            (State::Property, 'a'..='z' | '0'..='9' | '_') => {}
            (State::Property, '|') => state = State::PropertyStart,
            _ => return false,
        }
    }
    matches!(state, State::Property)
}

fn default_weight() -> f32 {
    1.0
}
