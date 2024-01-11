use super::{texture, Identifier, RightAngleRotation};
use crate::resource::manager::{get_resource_file, ResourceType};
use ahash::AHashMap;
use anyhow::{anyhow, bail, ensure, Context};
use bitfield::bitfield;
use nalgebra::{vector, Matrix4, Rotation3, Vector3};
use serde::Deserialize;
use std::rc::Rc;

#[derive(Debug)]
pub struct ModelCache {
    pub completed_models: AHashMap<Identifier, Rc<ModelType>>,
    pub templates: AHashMap<Identifier, Rc<Template>>,
}

impl ModelCache {
    pub fn new() -> Self {
        Self {
            completed_models: AHashMap::new(),
            templates: AHashMap::new(),
        }
    }

    #[tracing::instrument(skip(self, texture_atlas))]
    pub fn load_model(
        &mut self,
        location: &Identifier,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<Rc<ModelType>> {
        match self.get_or_load_model(location, texture_atlas, false)? {
            ModelState::Complete(model) => Ok(model),
            ModelState::Template(_) => Err(anyhow!("Model is a template, not a complete model")),
            ModelState::Pending => unreachable!(),
        }
    }

    #[tracing::instrument(skip(self, texture_atlas))]
    pub fn load_liquid(
        &mut self,
        location: &Identifier,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<Rc<ModelType>> {
        if let Some(model_type) = self.completed_models.get(location) {
            ensure!(
                matches!(model_type.as_ref(), &ModelType::Liquid(_)),
                "model previously loaded for {location:?} is not a liquid",
            );
            Ok(model_type.clone())
        } else {
            let json_bytes = get_resource_file(&ResourceType::Model, location)
                .with_context(|| format!("Failed to read raw model JSON data for {location:?}"))?;
            let model_template: Template = serde_json::from_slice(&json_bytes)
                .with_context(|| format!("Failed to parse model JSON data for {location:?}"))?;
            ensure!(
                model_template.parent.is_none(),
                "liquid model {location:?} must not have a parent"
            );
            ensure!(
                model_template.display.is_none(),
                "liquid model {location:?} must not have display data"
            );
            ensure!(
                model_template.elements.is_none(),
                "liquid model {location:?} must not have elements"
            );
            let texture_vars = model_template.texture_variables;
            ensure!(
                texture_vars.len() == 1 && texture_vars.contains_key("particle"),
                "liquid model {location:?} must contain one texture variable: `particle`",
            );
            let particle_identifier = Identifier::parse(&texture_vars["particle"])
                .context("Failed to parse identifier for \"particle\" texture variable")?;
            let uvs = texture_atlas
                .get_or_load_texture(&particle_identifier)
                .context("Failed to load \"particle\" texture")?
                .uvs;
            let model_info = Rc::new(ModelType::Liquid(LiquidInfo { uvs }));
            self.completed_models
                .insert(location.clone(), model_info.clone());
            Ok(model_info)
        }
    }

    #[tracing::instrument(skip(self, texture_atlas))]
    pub fn load_combined_model(
        &mut self,
        parts: &[CombinedModelPart],
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<Rc<ModelType>> {
        if parts.is_empty() {
            return Ok(Rc::new(ModelType::None));
        }
        let mut current_template: Option<Template> = None;
        for CombinedModelPart {
            x_rotation,
            y_rotation,
            location,
            uv_lock,
        } in parts
        {
            let template = match self.get_or_load_model(location, texture_atlas, true)? {
                ModelState::Complete(_) | ModelState::Pending => unreachable!(),
                ModelState::Template(template) => template,
            };
            let mut template = template.as_ref().clone();
            template.apply_blockstate_rotations(*x_rotation, *y_rotation, *uv_lock);
            if let Some(ref mut current_template) = current_template {
                if let Some(ref mut current_template_elements) = current_template.elements {
                    current_template_elements.extend(
                        template
                            .elements
                            .with_context(|| {
                                format!("combined model part {location:?} has no elements")
                            })?
                            .drain(..),
                    );
                }
            } else {
                current_template = Some(template);
            }
        }
        let mut final_template = current_template.unwrap();
        let complete = final_template.fill_texture_variables();
        if !complete {
            bail!("combined template {final_template:?} cannot be finalized");
        }
        Self::finalize_model(final_template, texture_atlas).map(Rc::new)
    }

    /// If `skip_finalizing_model` is specified, this will return the model's template instead of
    /// attempting to finalize and return the completed model.
    #[tracing::instrument(skip(self, texture_atlas))]
    fn get_or_load_model(
        &mut self,
        location: &Identifier,
        texture_atlas: &mut texture::AtlasBuilder,
        skip_finalizing_model: bool,
    ) -> anyhow::Result<ModelState> {
        if let (false, Some(model)) = (skip_finalizing_model, self.completed_models.get(location)) {
            return Ok(ModelState::Complete(model.clone()));
        }
        let (mut model_template, cached_template) = if let Some(template) =
            self.templates.get(location)
        {
            (template.as_ref().clone(), Some(template.clone()))
        } else {
            let json_bytes = get_resource_file(&ResourceType::Model, location)
                .with_context(|| format!("Failed to read raw model JSON data for {location:?}"))?;
            let mut model_template: Template = serde_json::from_slice(&json_bytes)
                .with_context(|| format!("Failed to parse model JSON data for {location:?}"))?;
            // If an element cullface is not specified, it should default to the cullface of that
            // side. This was a quick way to implement that without lengthy serde deserializer
            // replacement.
            if let Some(elements) = model_template.elements.as_mut() {
                for element in elements {
                    element.faces.autofill_empty_cullfaces();
                }
            }
            // Load parent, overlay child
            if let Some(parent_location) = &model_template.parent {
                let parent_identifier = Identifier::parse(parent_location)?;
                let parent = self
                    .get_or_load_model(&parent_identifier, texture_atlas, true)
                    .with_context(|| format!("Failed to load parent model of {location:?}"))?;
                match parent {
                    ModelState::Pending | ModelState::Complete(_) => unimplemented!(),
                    ModelState::Template(parent_template) => {
                        // Merge according to Mineraft Wiki rules:
                        // - Child elements override parent elements
                        // - Not stated(?), we assume child display also overrides parent
                        // - Ambient occlusion always taken from parent
                        model_template.elements = model_template
                            .elements
                            .or_else(|| parent_template.elements.clone());
                        model_template.display = model_template
                            .display
                            .or_else(|| parent_template.display.clone());
                        model_template.ambient_occlusion = parent_template.ambient_occlusion;
                        // Merge texture variables, child variables override parent variables.
                        // Mostly just required for the #particle texture variable.
                        let mut parent_texture_variables: AHashMap<_, _> = parent_template
                            .texture_variables
                            .iter()
                            .map(|(key, value)| {
                                if value.starts_with('#') {
                                    if let Some(replacement) =
                                        model_template.texture_variables.get(&value[1..])
                                    {
                                        return (key.clone(), replacement.clone());
                                    }
                                }
                                (key.clone(), value.clone())
                            })
                            .collect();
                        parent_texture_variables.extend(model_template.texture_variables);
                        model_template.texture_variables = parent_texture_variables;
                    }
                }
            }
            (model_template, None)
        };
        // HACK Remove self-referencing texture variables.
        // These are used in vanilla to have templates use child variables, but for us leaving the
        // variables dangling was simpler to implement.
        // Removing them is needed as some templates have variables like {"texture": "#texture"},
        // and when we go to fill in texture variables on the template, we haven't replaced
        // "texture" yet with the child variable's value, and so this just looks like a cycle,
        // which we don't deal with.
        model_template.texture_variables.retain(|variable, value| {
            // Remove all pairs in the form ("{variable}", "#{variable}")
            if value.len() != variable.len() + 1 {
                return true;
            }
            if !value.starts_with('#') {
                return true;
            }
            !value.ends_with(variable)
        });
        // Substitute texture variable references
        let complete = model_template.fill_texture_variables();
        let stored_template = if let Some(stored_template) = cached_template {
            stored_template
        } else {
            let stored_template = Rc::new(model_template.clone());
            assert!(
                self.templates
                    .insert(location.clone(), stored_template.clone())
                    .is_none(),
                "template already exists at {location:?}"
            );
            stored_template
        };
        // If template is complete, then we convert it to a finished model
        if !skip_finalizing_model && complete {
            let completed_model = Rc::new(Self::finalize_model(model_template, texture_atlas)?);
            assert!(
                self.completed_models
                    .insert(location.clone(), completed_model.clone())
                    .is_none(),
                "completed model already exists at {location:?}"
            );
            Ok(ModelState::Complete(completed_model))
        } else {
            Ok(ModelState::Template(stored_template))
        }
    }

    #[tracing::instrument(skip(texture_atlas))]
    fn finalize_model(
        model_template: Template,
        texture_atlas: &mut texture::AtlasBuilder,
    ) -> anyhow::Result<ModelType> {
        // Try various specialisations on model
        'specialize_empty: {
            if model_template
                .elements
                .as_ref()
                .map(|elements| elements.len() > 0)
                .unwrap_or(false)
            {
                break 'specialize_empty;
            }
            return Ok(ModelType::None);
        }
        'specialize_block: {
            // Blocks have a single element spanning the entire block space with no
            // rotation
            let elements = model_template.elements.as_ref().unwrap();
            let [element] = &elements[..] else {
                break 'specialize_block;
            };
            if element.start_pos != vector![0., 0., 0.] || element.end_pos != vector![16., 16., 16.]
            {
                break 'specialize_block;
            }
            if element.rotation.is_some() || element.blockstate_rotation.is_some() {
                break 'specialize_block;
            }
            if element.faces.iter().any(|(_, face)| face.is_none()) {
                break 'specialize_block;
            }
            if element.blockstate_rotation.is_some() {
                todo!("Support blockstate rotation for standard blocks")
            }
            // Load textures
            let per_face_atlas_uvs = [
                Self::load_face_atlas_uvs(texture_atlas, element.faces.top.as_ref().unwrap())?,
                Self::load_face_atlas_uvs(texture_atlas, element.faces.bottom.as_ref().unwrap())?,
                Self::load_face_atlas_uvs(texture_atlas, element.faces.north.as_ref().unwrap())?,
                Self::load_face_atlas_uvs(texture_atlas, element.faces.south.as_ref().unwrap())?,
                Self::load_face_atlas_uvs(texture_atlas, element.faces.east.as_ref().unwrap())?,
                Self::load_face_atlas_uvs(texture_atlas, element.faces.west.as_ref().unwrap())?,
            ];
            let mut flags = BlockFlags(0);
            flags.set_ambient_occlusion(model_template.ambient_occlusion);
            return Ok(ModelType::Block(BlockInfo {
                flags,
                per_face_atlas_uvs,
            }));
        }
        'specialize_overlayed_block: {
            // Overlayed blocks have a single element spanning the entire block space with
            // no rotation
            let elements = model_template.elements.as_ref().unwrap();
            let [base_element, overlay_element] = &elements[..] else {
                break 'specialize_overlayed_block;
            };
            if base_element.start_pos != vector![0.0, 0.0, 0.0]
                || base_element.end_pos != vector![16.0, 16.0, 16.0]
            {
                break 'specialize_overlayed_block;
            }
            if overlay_element.start_pos != vector![0.0, 0.0, 0.0]
                || overlay_element.end_pos != vector![16.0, 16.0, 16.0]
            {
                break 'specialize_overlayed_block;
            }
            if base_element.rotation.is_some() || overlay_element.rotation.is_some() {
                break 'specialize_overlayed_block;
            }
            if base_element.faces.iter().any(|(_, face)| face.is_none()) {
                break 'specialize_overlayed_block;
            }
            if base_element.blockstate_rotation.is_some()
                || overlay_element.blockstate_rotation.is_some()
            {
                todo!("Support blockstate rotation for overlayed blocks")
            }
            // TODO The functions for these are already done, just port over from standard block
            // specialization
            if base_element
                .faces
                .iter()
                .filter_map(|(i, f)| f.map(|f| (i, f)))
                .any(|(_, face)| face.rotation != RightAngleRotation::Zero)
            {
                todo!("Support block face texture rotation");
            }
            if overlay_element
                .faces
                .iter()
                .filter_map(|(i, f)| f.map(|f| (i, f)))
                .any(|(_, face)| face.rotation != RightAngleRotation::Zero)
            {
                todo!("Support block face texture rotation");
            }
            if base_element
                .faces
                .iter()
                .filter_map(|(i, f)| f.map(|f| (i, f)))
                .any(|(_, face)| face.uvs.is_some() && face.uvs != Some([0.0, 0.0, 16.0, 16.0]))
            {
                todo!("Support block face custom uvs");
            }
            if overlay_element
                .faces
                .iter()
                .filter_map(|(i, f)| f.map(|f| (i, f)))
                .any(|(_, face)| face.uvs.is_some() && face.uvs != Some([0.0, 0.0, 16.0, 16.0]))
            {
                todo!("Support block face custom uvs");
            }
            // Load textures
            let per_base_face_atlas_uvs = [
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.top.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.bottom.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.north.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.south.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.east.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
                texture_atlas
                    .get_or_load_texture(&Identifier::parse(
                        &base_element.faces.west.as_ref().unwrap().texture,
                    )?)?
                    .uvs,
            ];
            let mut flags = OverlayedBlockFlags(0);
            flags.set_ambient_occlusion(model_template.ambient_occlusion);
            let mut overlay_uvs: [Option<[u16; 4]>; 6] = [None; 6];
            for (index, overlay_face) in overlay_element.faces.iter() {
                if let Some(overlay_face) = overlay_face {
                    overlay_uvs[index as usize] = Some(
                        texture_atlas
                            .get_or_load_texture(&Identifier::parse(&overlay_face.texture)?)?
                            .uvs,
                    );
                    match index {
                        BlockFace::Top => flags.set_overlay_top(true),
                        BlockFace::Bottom => flags.set_overlay_bottom(true),
                        BlockFace::North => flags.set_overlay_north(true),
                        BlockFace::South => flags.set_overlay_south(true),
                        BlockFace::East => flags.set_overlay_east(true),
                        BlockFace::West => flags.set_overlay_west(true),
                    }
                }
            }
            return Ok(ModelType::OverlayedBlock(OverlayedBlockInfo {
                flags,
                per_base_face_atlas_uvs,
                per_overlay_face_atlas_uvs: overlay_uvs.map(|uv| uv.unwrap_or([0xFFFF; 4])),
            }));
        }
        'specialize_cross: {
            // Blocks have a single element spanning the entire block space with no
            // rotation
            let elements = model_template.elements.as_ref().unwrap();
            let [face_1, face_2] = &elements[..] else {
                break 'specialize_cross;
            };
            if face_1.shade || face_2.shade {
                break 'specialize_cross;
            }
            if face_1.start_pos != vector![0.8, 0., 8.] || face_1.end_pos != vector![15.2, 16., 8.]
            {
                break 'specialize_cross;
            }
            if face_2.start_pos != vector![8., 0., 0.8] || face_2.end_pos != vector![8., 16., 15.2]
            {
                break 'specialize_cross;
            }
            if face_1.rotation
                != Some(ModelElementRotation {
                    origin: vector![8., 8., 8.],
                    axis: RotationAxis::Y,
                    angle: 45.,
                    rescale: true,
                })
            {
                break 'specialize_cross;
            }
            if face_2.rotation
                != Some(ModelElementRotation {
                    origin: vector![8., 8., 8.],
                    axis: RotationAxis::Y,
                    angle: 45.,
                    rescale: true,
                })
            {
                break 'specialize_cross;
            }
            if face_1.blockstate_rotation.is_some() || face_2.blockstate_rotation.is_some() {
                todo!("blockstate rotation on a cross? (probably just change todo to break)");
                // break 'specialize_cross;
            }
            let Some(ref north_face) = face_1.faces.north else {
                break 'specialize_cross;
            };
            let cross_texture = &north_face.texture;
            if !matches!(&face_1.faces, TemplateElementFaces {
                    top: None,
                    bottom: None,
                    north: Some(TemplateElementFace {
                        uvs: Some([n_uv1, n_uv2, n_uv3, n_uv4]),
                        texture: north_texture,
                        cullface: Some(BlockFace::North),
                        rotation: RightAngleRotation::Zero,
                        tint_index: -1,
                    }),
                    south: Some(TemplateElementFace {
                        uvs: Some([s_uv1, s_uv2, s_uv3, s_uv4]),
                        texture: south_texture,
                        cullface: Some(BlockFace::South),
                        rotation: RightAngleRotation::Zero,
                        tint_index: -1,
                    }),
                    east: None,
                    west: None,
                } if &north_texture == &cross_texture &&
                    &south_texture == &cross_texture &&
                    *n_uv1 == 0.0 &&
                    *n_uv2 == 0.0 &&
                    *n_uv3 == 16.0 &&
                    *n_uv4 == 16.0 &&
                    *s_uv1 == 0.0 &&
                    *s_uv2 == 0.0 &&
                    *s_uv3 == 16.0 &&
                    *s_uv4 == 16.0)
            {
                break 'specialize_cross;
            }
            if !matches!(&face_2.faces, TemplateElementFaces {
                    top: None,
                    bottom: None,
                    north: None,
                    south: None,
                    east: Some(TemplateElementFace {
                        uvs: Some([e_uv1, e_uv2, e_uv3, e_uv4]),
                        texture: east_texture,
                        cullface: Some(BlockFace::East),
                        rotation: RightAngleRotation::Zero,
                        tint_index: -1,
                    }),
                    west: Some(TemplateElementFace {
                        uvs: Some([w_uv1, w_uv2, w_uv3, w_uv4]),
                        texture: west_texture,
                        cullface: Some(BlockFace::West),
                        rotation: RightAngleRotation::Zero,
                        tint_index: -1,
                    }),
                } if &east_texture == &cross_texture &&
                    &west_texture == &cross_texture &&
                    *e_uv1 == 0.0 &&
                    *e_uv2 == 0.0 &&
                    *e_uv3 == 16.0 &&
                    *e_uv4 == 16.0 &&
                    *w_uv1 == 0.0 &&
                    *w_uv2 == 0.0 &&
                    *w_uv3 == 16.0 &&
                    *w_uv4 == 16.0)
            {
                break 'specialize_cross;
            }
            // Load texture
            let cross_atlas_start_uvs = texture_atlas
                .get_or_load_texture(&Identifier::parse(&cross_texture)?)?
                .uvs;
            return Ok(ModelType::Cross(CrossInfo {
                cross_atlas_start_uvs,
            }));
        }
        'specialize_biome_tinted_cross: {
            // Blocks have a single element spanning the entire block space with no
            // rotation
            let elements = model_template.elements.as_ref().unwrap();
            let [face_1, face_2] = &elements[..] else {
                break 'specialize_biome_tinted_cross;
            };
            if face_1.shade || face_2.shade {
                break 'specialize_biome_tinted_cross;
            }
            if face_1.start_pos != vector![0.8, 0., 8.] || face_1.end_pos != vector![15.2, 16., 8.]
            {
                break 'specialize_biome_tinted_cross;
            }
            if face_2.start_pos != vector![8., 0., 0.8] || face_2.end_pos != vector![8., 16., 15.2]
            {
                break 'specialize_biome_tinted_cross;
            }
            if face_1.rotation
                != Some(ModelElementRotation {
                    origin: vector![8., 8., 8.],
                    axis: RotationAxis::Y,
                    angle: 45.,
                    rescale: true,
                })
            {
                break 'specialize_biome_tinted_cross;
            }
            if face_2.rotation
                != Some(ModelElementRotation {
                    origin: vector![8., 8., 8.],
                    axis: RotationAxis::Y,
                    angle: 45.,
                    rescale: true,
                })
            {
                break 'specialize_biome_tinted_cross;
            }
            if face_1.blockstate_rotation.is_some() || face_2.blockstate_rotation.is_some() {
                todo!("blockstate rotation on a cross? (probably just change todo to break)");
                // break 'specialize_biome_tinted_cross;
            }
            let Some(ref north_face) = face_1.faces.north else {
                break 'specialize_biome_tinted_cross;
            };
            let cross_texture = &north_face.texture;
            if !matches!(&face_1.faces, TemplateElementFaces {
                    top: None,
                    bottom: None,
                    north: Some(TemplateElementFace {
                        uvs: Some([n_uv1, n_uv2, n_uv3, n_uv4]),
                        texture: north_texture,
                        cullface: Some(BlockFace::North),
                        rotation: RightAngleRotation::Zero,
                        tint_index: 0,
                    }),
                    south: Some(TemplateElementFace {
                        uvs: Some([s_uv1, s_uv2, s_uv3, s_uv4]),
                        texture: south_texture,
                        cullface: Some(BlockFace::South),
                        rotation: RightAngleRotation::Zero,
                        tint_index: 0,
                    }),
                    east: None,
                    west: None,
                } if &north_texture == &cross_texture &&
                    &south_texture == &cross_texture &&
                    *n_uv1 == 0.0 &&
                    *n_uv2 == 0.0 &&
                    *n_uv3 == 16.0 &&
                    *n_uv4 == 16.0 &&
                    *s_uv1 == 0.0 &&
                    *s_uv2 == 0.0 &&
                    *s_uv3 == 16.0 &&
                    *s_uv4 == 16.0)
            {
                break 'specialize_biome_tinted_cross;
            }
            if !matches!(&face_2.faces, TemplateElementFaces {
                    top: None,
                    bottom: None,
                    north: None,
                    south: None,
                    east: Some(TemplateElementFace {
                        uvs: Some([e_uv1, e_uv2, e_uv3, e_uv4]),
                        texture: east_texture,
                        cullface: Some(BlockFace::East),
                        rotation: RightAngleRotation::Zero,
                        tint_index: 0,
                    }),
                    west: Some(TemplateElementFace {
                        uvs: Some([w_uv1, w_uv2, w_uv3, w_uv4]),
                        texture: west_texture,
                        cullface: Some(BlockFace::West),
                        rotation: RightAngleRotation::Zero,
                        tint_index: 0,
                    }),
                } if &east_texture == &cross_texture &&
                    &west_texture == &cross_texture &&
                    *e_uv1 == 0.0 &&
                    *e_uv2 == 0.0 &&
                    *e_uv3 == 16.0 &&
                    *e_uv4 == 16.0 &&
                    *w_uv1 == 0.0 &&
                    *w_uv2 == 0.0 &&
                    *w_uv3 == 16.0 &&
                    *w_uv4 == 16.0)
            {
                break 'specialize_biome_tinted_cross;
            }
            // Load texture
            let cross_atlas_start_uvs = texture_atlas
                .get_or_load_texture(&Identifier::parse(&cross_texture)?)?
                .uvs;
            return Ok(ModelType::BiomeTintedCross(CrossInfo {
                cross_atlas_start_uvs,
            }));
        }
        // Fall back to more expensive model rendering if we can't specialize
        let converted_elements: Vec<ModelElement> = model_template
            .elements
            .unwrap()
            .into_iter()
            .map(|template_element| {
                // Generate matrix which transforms normal block faces to element faces
                let matrix = {
                    /// Converts a Minecraft element coordinate to a model coordinate.
                    /// Minecraft element coordinates are 0 to 16 within a block, whereas
                    /// we have model coordinates from -1 to +1.
                    fn mc_elem_to_model_coord(vec: Vector3<f32>) -> Vector3<f32> {
                        vec.add_scalar(-8.0) / 8.0
                    }
                    let start = mc_elem_to_model_coord(template_element.start_pos);
                    let end = mc_elem_to_model_coord(template_element.end_pos);
                    let size = end - start;
                    let origin = (start + end) / 2.0;
                    let rotation = match template_element.rotation {
                        None => Matrix4::identity(),
                        Some(template_rotation) => {
                            let origin = mc_elem_to_model_coord(template_rotation.origin);
                            let axis = match template_rotation.axis {
                                RotationAxis::X => Vector3::x_axis(),
                                RotationAxis::Y => Vector3::y_axis(),
                                RotationAxis::Z => Vector3::z_axis(),
                            };
                            let angle = template_rotation.angle.to_radians();
                            let rotation = Matrix4::from_axis_angle(&axis, angle)
                                .prepend_translation(&-origin)
                                .append_translation(&origin);
                            let rescaled_rotation = if template_rotation.rescale {
                                let rescale_amount = 1.0 + (1.0 / (angle.cos() - 1.0));
                                match template_rotation.axis {
                                    RotationAxis::X => rotation.append_nonuniform_scaling(
                                        &Vector3::new(1.0, rescale_amount, rescale_amount),
                                    ),
                                    RotationAxis::Y => rotation.append_nonuniform_scaling(
                                        &Vector3::new(rescale_amount, 1.0, rescale_amount),
                                    ),
                                    RotationAxis::Z => rotation.append_nonuniform_scaling(
                                        &Vector3::new(rescale_amount, rescale_amount, 1.0),
                                    ),
                                }
                            } else {
                                rotation
                            };
                            match template_element.blockstate_rotation {
                                None => rescaled_rotation,
                                Some(TemplateElementBlockstateRotation {
                                    x_rotation,
                                    y_rotation,
                                    ..
                                }) => {
                                    let x_y_axis_angles = [
                                        (Vector3::x_axis(), x_rotation),
                                        (Vector3::y_axis(), y_rotation),
                                    ];
                                    let [x_blockstate_rot, y_blockstate_rot] =
                                        x_y_axis_angles.map(|(axis, angle)| match angle {
                                            RightAngleRotation::Zero => Matrix4::identity(),
                                            RightAngleRotation::Ninety => {
                                                Rotation3::from_axis_angle(
                                                    &axis,
                                                    std::f32::consts::FRAC_PI_2,
                                                )
                                                .to_homogeneous()
                                            }
                                            RightAngleRotation::OneEighty => {
                                                Rotation3::from_axis_angle(
                                                    &axis,
                                                    std::f32::consts::PI,
                                                )
                                                .to_homogeneous()
                                            }
                                            RightAngleRotation::TwoSeventy => {
                                                Rotation3::from_axis_angle(
                                                    &axis,
                                                    -std::f32::consts::FRAC_PI_2,
                                                )
                                                .to_homogeneous()
                                            }
                                        });
                                    y_blockstate_rot * x_blockstate_rot * rescaled_rotation
                                }
                            }
                        }
                    };
                    rotation
                        .append_nonuniform_scaling(&size)
                        .append_translation(&origin)
                };
                // Convert template faces
                let faces = {
                    let faces = template_element.faces;
                    let rot = template_element.blockstate_rotation;
                    ModelElementFaces {
                        top: Self::convert_face(texture_atlas, faces.top, rot, BlockFace::Top)?,
                        bottom: Self::convert_face(
                            texture_atlas,
                            faces.bottom,
                            rot,
                            BlockFace::Bottom,
                        )?,
                        north: Self::convert_face(
                            texture_atlas,
                            faces.north,
                            rot,
                            BlockFace::North,
                        )?,
                        south: Self::convert_face(
                            texture_atlas,
                            faces.south,
                            rot,
                            BlockFace::South,
                        )?,
                        east: Self::convert_face(texture_atlas, faces.east, rot, BlockFace::East)?,
                        west: Self::convert_face(texture_atlas, faces.west, rot, BlockFace::West)?,
                    }
                };
                Ok(ModelElement {
                    matrix,
                    shade: template_element.shade,
                    faces,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(ModelType::Other(OtherInfo {
            elements: converted_elements,
        }));
    }

    /// Rotates `texture_uvs` around its centre by `angle`.
    fn rotate_uvs(texture_uvs: [u16; 4], angle: RightAngleRotation) -> [u16; 4] {
        let uvs_f32 = texture_uvs.map(|uv| uv as f32);
        let midpoint_x = (uvs_f32[0] + uvs_f32[2]) / 2.0;
        let midpoint_y = (uvs_f32[1] + uvs_f32[3]) / 2.0;
        let tranformed_uvs = [
            uvs_f32[0] - midpoint_x,
            uvs_f32[1] - midpoint_y,
            uvs_f32[2] - midpoint_x,
            uvs_f32[3] - midpoint_y,
        ];
        let transformed_rotated_uvs = match angle {
            RightAngleRotation::Zero => tranformed_uvs,
            RightAngleRotation::Ninety => [
                -tranformed_uvs[1],
                tranformed_uvs[0],
                -tranformed_uvs[3],
                tranformed_uvs[2],
            ],
            RightAngleRotation::OneEighty => [
                -tranformed_uvs[0],
                -tranformed_uvs[1],
                -tranformed_uvs[2],
                -tranformed_uvs[3],
            ],
            RightAngleRotation::TwoSeventy => [
                tranformed_uvs[1],
                -tranformed_uvs[0],
                tranformed_uvs[3],
                -tranformed_uvs[2],
            ],
        };
        [
            (transformed_rotated_uvs[0] + midpoint_x) as u16,
            (transformed_rotated_uvs[1] + midpoint_y) as u16,
            (transformed_rotated_uvs[2] + midpoint_x) as u16,
            (transformed_rotated_uvs[3] + midpoint_y) as u16,
        ]
    }

    /// Transforms UVs from the texture map using element UVs (0 to 16).
    fn transform_uvs(texture_uvs: [u16; 4], element_uvs: [f32; 4]) -> [u16; 4] {
        let start_x = texture_uvs[0] as f32;
        let x_diff = texture_uvs[2] as f32 - start_x;
        let start_y = texture_uvs[1] as f32;
        let y_diff = texture_uvs[3] as f32 - start_y;
        let element_uvs = element_uvs.map(|x| {
            assert!(x <= 16.0);
            x / 16.0
        });
        [
            (start_x + (x_diff * element_uvs[0])).round() as u16,
            (start_y + (y_diff * element_uvs[1])).round() as u16,
            (start_x + (x_diff * element_uvs[2])).round() as u16,
            (start_y + (y_diff * element_uvs[3])).round() as u16,
        ]
    }

    #[tracing::instrument(skip(atlas))]
    fn load_face_atlas_uvs(
        atlas: &mut texture::AtlasBuilder,
        face: &TemplateElementFace,
    ) -> anyhow::Result<[u16; 4]> {
        let base_uvs = atlas
            .get_or_load_texture(&Identifier::parse(&face.texture)?)?
            .uvs;
        let custom_uvs = match face.uvs {
            None => base_uvs,
            Some(uvs) => {
                let x_start = base_uvs[0] as f32;
                let x_diff = (base_uvs[2] - base_uvs[0]) as f32;
                let y_start = base_uvs[1] as f32;
                let y_diff = (base_uvs[3] - base_uvs[1]) as f32;
                let uv_fractions = uvs.map(|uv| uv as f32 / 16.0);
                [
                    (uv_fractions[0] * x_diff + x_start) as u16,
                    (uv_fractions[1] * y_diff + y_start) as u16,
                    (uv_fractions[0] * x_diff + x_start) as u16,
                    (uv_fractions[1] * y_diff + y_start) as u16,
                ]
            }
        };
        let rotated_uvs = Self::rotate_uvs(custom_uvs, face.rotation);
        Ok(rotated_uvs)
    }

    fn convert_face(
        texture_atlas: &mut texture::AtlasBuilder,
        face: Option<TemplateElementFace>,
        blockstate_rotation: Option<TemplateElementBlockstateRotation>,
        index: BlockFace,
    ) -> anyhow::Result<Option<ModelElementFace>> {
        let Some(face) = face else { return Ok(None) };
        let texture_uvs = texture_atlas
            .get_or_load_texture(&Identifier::parse(&face.texture)?)?
            .uvs;
        let transformed_uvs = match face.uvs {
            None => texture_uvs,
            Some(uvs) => Self::transform_uvs(texture_uvs, uvs),
        };
        let rotated_uvs = match blockstate_rotation {
            Some(TemplateElementBlockstateRotation {
                x_rotation,
                y_rotation,
                uv_lock,
            }) if uv_lock => {
                // FIXME Pretty sure this is rotating some faces the wrong way. Check using a
                // dropper.
                match index {
                    BlockFace::Top => Self::rotate_uvs(transformed_uvs, y_rotation),
                    BlockFace::Bottom => Self::rotate_uvs(transformed_uvs, y_rotation),
                    BlockFace::East => Self::rotate_uvs(transformed_uvs, x_rotation),
                    BlockFace::West => Self::rotate_uvs(transformed_uvs, x_rotation),
                    // See above todo!
                    _ => transformed_uvs,
                }
            }
            _ => transformed_uvs,
        };
        Ok(Some(ModelElementFace {
            uvs: rotated_uvs,
            cullface: face.cullface.unwrap_or(index),
            tint: match face.tint_index {
                -1 => None,
                // Apparently vanilla only uses one tint index, so anything other than -1 just
                // means `Tint::Biome`
                _ => Some(Tint::Biome),
                // _ => unimplemented!("unknown tint index {}", face.tint_index),
            },
        }))
    }
}

#[derive(Clone, Debug)]
pub struct CombinedModelPart {
    pub location: Identifier,
    pub x_rotation: RightAngleRotation,
    pub y_rotation: RightAngleRotation,
    pub uv_lock: bool,
}

// TODO Change Pending to have a Rayon ThreadPool thread index, implement parallel model loading
// // FIXME This method of loading models does not detect cycles between threads, so freezes can occur
// // if assets contain cycles.

#[derive(Clone, Debug)]
pub enum ModelState {
    Complete(Rc<ModelType>),
    Template(Rc<Template>),
    /// Model is currently being loaded
    Pending,
}

#[derive(Clone, Debug)]
pub enum ModelType {
    /// Model with no elements. Example: Air
    None,
    /// Standard block, has all six faces, only element, and no fancy stuff. Example: Cobblestone
    Block(BlockInfo),
    /// Block with extra faces containing transparent pixels drawn on top. Example: Grass block
    OverlayedBlock(OverlayedBlockInfo),
    /// Contains two double sided 2D elements at 45 degrees. Example: Dandelion
    Cross(CrossInfo),
    /// Contains two double sided biome tinted 2D elements at 45 degrees. Example: Grass
    BiomeTintedCross(CrossInfo),
    /// Hardcoded rendering, faces dynamically generated. Example: Water
    Liquid(LiquidInfo),
    /// Any other type of model, unspecialized. Example: Farmland
    Other(OtherInfo),
}

#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub flags: BlockFlags,
    /// In order of top, bottom, north, south, east and west.
    pub per_face_atlas_uvs: [[u16; 4]; 6],
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct BlockFlags(u8);
    impl Debug;
    pub ambient_occlusion, set_ambient_occlusion: 0;
}

#[derive(Clone, Copy, Debug)]
pub struct OverlayedBlockInfo {
    pub flags: OverlayedBlockFlags,
    /// In order of top, bottom, north, south, east and west.
    pub per_base_face_atlas_uvs: [[u16; 4]; 6],
    pub per_overlay_face_atlas_uvs: [[u16; 4]; 6],
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct OverlayedBlockFlags(u16);
    impl Debug;
    pub ambient_occlusion, set_ambient_occlusion: 0;
    pub has_overlay_top, set_overlay_top: 1;
    pub has_overlay_bottom, set_overlay_bottom: 2;
    pub has_overlay_north, set_overlay_north: 3;
    pub has_overlay_south, set_overlay_south: 4;
    pub has_overlay_east, set_overlay_east: 5;
    pub has_overlay_west, set_overlay_west: 6;
}

#[derive(Clone, Copy, Debug)]
pub struct CrossInfo {
    pub cross_atlas_start_uvs: [u16; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct LiquidInfo {
    pub uvs: [u16; 4],
}

#[derive(Clone, Debug)]
pub struct OtherInfo {
    pub elements: Vec<ModelElement>,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelElement {
    pub matrix: Matrix4<f32>,
    pub shade: bool,
    pub faces: ModelElementFaces,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelElementFaces {
    pub top: Option<ModelElementFace>,
    pub bottom: Option<ModelElementFace>,
    pub north: Option<ModelElementFace>,
    pub south: Option<ModelElementFace>,
    pub east: Option<ModelElementFace>,
    pub west: Option<ModelElementFace>,
}

#[derive(Clone, Copy, Debug)]
pub struct ModelElementFace {
    pub uvs: [u16; 4],
    pub cullface: BlockFace,
    pub tint: Option<Tint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Biome,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Template {
    pub parent: Option<String>,
    #[serde(default = "bool_true", rename = "ambientocclusion")]
    pub ambient_occlusion: bool,
    pub display: Option<ModelDisplay>,
    #[serde(default, rename = "textures")]
    pub texture_variables: AHashMap<String, String>,
    pub elements: Option<Vec<TemplateElement>>,
}

impl Template {
    // NOTE This is copied further up for CombinedTemplate
    /// Substitutes references to texture variables in elements repeatedly using its texture
    /// variables.
    /// Does not check for cycles in its map.
    /// Returns whether the template is complete (either has elements, or all texture variables are
    /// replaced by paths).
    pub fn fill_texture_variables(&mut self) -> bool {
        let mut all_replaced = true;
        match &mut self.elements {
            Some(elements) => {
                for element in elements {
                    let replaced = element
                        .faces
                        .fill_texture_variables(&mut self.texture_variables);
                    all_replaced = all_replaced && replaced;
                }
            }
            None => {}
        }
        all_replaced
    }

    /// Applies blockstate rotations `x_rotation` and `y_rotation` to every element in the
    /// template.
    #[tracing::instrument(skip(self))]
    pub fn apply_blockstate_rotations(
        &mut self,
        x_rotation: RightAngleRotation,
        y_rotation: RightAngleRotation,
        uv_lock: bool,
    ) {
        if let Some(ref mut elements) = self.elements {
            for element in elements {
                assert!(element.blockstate_rotation.is_none());
                element.blockstate_rotation = Some(TemplateElementBlockstateRotation {
                    x_rotation,
                    y_rotation,
                    uv_lock,
                });
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ModelDisplay {
    pub thirdperson_righthand: Option<ModelDisplayPosition>,
    pub thirdperson_lefthand: Option<ModelDisplayPosition>,
    pub firstperson_righthand: Option<ModelDisplayPosition>,
    pub firstperson_lefthand: Option<ModelDisplayPosition>,
    pub gui: Option<ModelDisplayPosition>,
    pub head: Option<ModelDisplayPosition>,
    pub ground: Option<ModelDisplayPosition>,
    pub fixed: Option<ModelDisplayPosition>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ModelDisplayPosition {
    #[serde(default)]
    pub rotation: Vector3<f32>,
    #[serde(default)]
    pub translation: Vector3<f32>,
    #[serde(default)]
    pub scale: Vector3<f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TemplateElement {
    #[serde(rename = "from")]
    pub start_pos: Vector3<f32>,
    #[serde(rename = "to")]
    pub end_pos: Vector3<f32>,
    pub rotation: Option<ModelElementRotation>,
    #[serde(default = "bool_true")]
    pub shade: bool,
    pub faces: TemplateElementFaces,
    pub blockstate_rotation: Option<TemplateElementBlockstateRotation>,
}

/// Variant information taken from parent blockstate.
/// Passed through template elements as templates may be multiple models combined.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct TemplateElementBlockstateRotation {
    pub x_rotation: RightAngleRotation,
    pub y_rotation: RightAngleRotation,
    pub uv_lock: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct ModelElementRotation {
    pub origin: Vector3<f32>,
    pub angle: f32,
    pub axis: RotationAxis,
    #[serde(default)]
    pub rescale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RotationAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TemplateElementFaces {
    #[serde(rename = "up")]
    pub top: Option<TemplateElementFace>,
    #[serde(rename = "down")]
    pub bottom: Option<TemplateElementFace>,
    pub north: Option<TemplateElementFace>,
    pub south: Option<TemplateElementFace>,
    pub east: Option<TemplateElementFace>,
    pub west: Option<TemplateElementFace>,
}

impl TemplateElementFaces {
    pub fn autofill_empty_cullfaces(&mut self) {
        if let Some(top) = self.top.as_mut() {
            top.cullface = Some(top.cullface.unwrap_or(BlockFace::Top))
        }
        if let Some(bottom) = self.bottom.as_mut() {
            bottom.cullface = Some(bottom.cullface.unwrap_or(BlockFace::Bottom))
        }
        if let Some(north) = self.north.as_mut() {
            north.cullface = Some(north.cullface.unwrap_or(BlockFace::North))
        }
        if let Some(south) = self.south.as_mut() {
            south.cullface = Some(south.cullface.unwrap_or(BlockFace::South))
        }
        if let Some(east) = self.east.as_mut() {
            east.cullface = Some(east.cullface.unwrap_or(BlockFace::East))
        }
        if let Some(west) = self.west.as_mut() {
            west.cullface = Some(west.cullface.unwrap_or(BlockFace::West))
        }
    }

    /// Substitutes references to texture variables repeatedly using the provided map.
    /// Does not check for cycles in the map.
    /// Returns whether all remaining references to texture variables are now replaced.
    pub fn fill_texture_variables(&mut self, variable_map: &mut AHashMap<String, String>) -> bool {
        let mut all_replaced = true;
        for (_, face) in self.iter_mut().filter_map(|(i, f)| f.map(|f| (i, f))) {
            let replaced = face.fill_texture_variable(variable_map);
            all_replaced = all_replaced && replaced;
        }
        all_replaced
    }

    pub fn iter(&self) -> TemplateElementFacesIterator<&TemplateElementFace> {
        TemplateElementFacesIterator {
            faces: [
                (BlockFace::Top, self.top.as_ref()),
                (BlockFace::Bottom, self.bottom.as_ref()),
                (BlockFace::North, self.north.as_ref()),
                (BlockFace::South, self.south.as_ref()),
                (BlockFace::East, self.east.as_ref()),
                (BlockFace::West, self.west.as_ref()),
            ],
            current_face: 0,
        }
    }

    pub fn iter_mut(&mut self) -> TemplateElementFacesIterator<&mut TemplateElementFace> {
        TemplateElementFacesIterator {
            faces: [
                (BlockFace::Top, self.top.as_mut()),
                (BlockFace::Bottom, self.bottom.as_mut()),
                (BlockFace::North, self.north.as_mut()),
                (BlockFace::South, self.south.as_mut()),
                (BlockFace::East, self.east.as_mut()),
                (BlockFace::West, self.west.as_mut()),
            ],
            current_face: 0,
        }
    }
}

// HACK Couldn't figure out the lifetimes trying to reference the container, so went with this
// instead
#[derive(Debug)]
pub struct TemplateElementFacesIterator<T> {
    faces: [(BlockFace, Option<T>); 6],
    current_face: usize,
}

impl<T> Iterator for TemplateElementFacesIterator<T> {
    type Item = (BlockFace, Option<T>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_face >= 6 {
            return None;
        }
        let index = self.faces[self.current_face].0;
        let face = self.faces[self.current_face].1.take();
        self.current_face += 1;
        Some((index, face))
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TemplateElementFace {
    #[serde(rename = "uv")]
    pub uvs: Option<[f32; 4]>,
    pub texture: String,
    pub cullface: Option<BlockFace>,
    #[serde(default)]
    pub rotation: RightAngleRotation,
    #[serde(default = "tint_index_none", rename = "tintindex")]
    pub tint_index: i16,
}

impl TemplateElementFace {
    /// Substitutes references to texture variables repeatedly using the provided map.
    /// Does not check for cycles in the map.
    /// Returns whether this doesn't refer to a texture variable (now or already).
    pub fn fill_texture_variable(&mut self, variable_map: &mut AHashMap<String, String>) -> bool {
        loop {
            if !self.texture.starts_with('#') {
                return true;
            }
            match variable_map.get(&self.texture[1..]) {
                None => return false,
                Some(new_value) => {
                    self.texture = new_value.clone();
                    continue;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum BlockFace {
    #[serde(rename = "up")]
    Top = 0,
    #[serde(rename = "down", alias = "bottom")]
    Bottom = 1,
    North = 2,
    South = 3,
    East = 4,
    West = 5,
}

fn bool_true() -> bool {
    true
}

fn tint_index_none() -> i16 {
    -1
}
