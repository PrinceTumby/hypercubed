use super::{texture, Identifier, RightAngleRotation};
use crate::resource::manager::{get_resource_file, ResourceType};
use ahash::AHashMap;
use anyhow::{anyhow, bail, ensure, Context};
use bitfield::bitfield;
use nalgebra::{point, Matrix4, Point3, Rotation3, Vector3};
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
            if element.start_pos != point![0., 0., 0.] || element.end_pos != point![16., 16., 16.] {
                break 'specialize_block;
            }
            if element.rotation.is_some() || element.blockstate_rotation.is_some() {
                break 'specialize_block;
            }
            if element.faces.iter().any(|(_, face)| face.is_none()) {
                break 'specialize_block;
            }
            if element
                .faces
                .iter()
                .any(|(_, face)| face.map(|f| f.tint_index != -1).unwrap_or(false))
            {
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
        'specialize_tinted_block: {
            // Blocks have a single element spanning the entire block space with no
            // rotation
            let elements = model_template.elements.as_ref().unwrap();
            let [element] = &elements[..] else {
                break 'specialize_tinted_block;
            };
            if element.start_pos != point![0., 0., 0.] || element.end_pos != point![16., 16., 16.] {
                break 'specialize_tinted_block;
            }
            if element.rotation.is_some() || element.blockstate_rotation.is_some() {
                break 'specialize_tinted_block;
            }
            if element.faces.iter().any(|(_, face)| face.is_none()) {
                break 'specialize_tinted_block;
            }
            if element
                .faces
                .iter()
                .any(|(_, face)| face.map(|f| f.tint_index == -1).unwrap_or(false))
            {
                break 'specialize_tinted_block;
            }
            if element.blockstate_rotation.is_some() {
                todo!("Support blockstate rotation for tinted blocks")
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
            return Ok(ModelType::TintedBlock(BlockInfo {
                flags,
                per_face_atlas_uvs,
            }));
        }
        'specialize_overlayed_block: {
            // Overlayed blocks have any number of elements, all spanning the entire block space,
            // none of which with rotation
            let elements = model_template.elements.as_ref().unwrap();
            if elements.iter().any(|element| {
                element.start_pos != point![0.0, 0.0, 0.0]
                    || element.end_pos != point![16.0, 16.0, 16.0]
            }) {
                break 'specialize_overlayed_block;
            }
            if elements.iter().any(|element| element.rotation.is_some()) {
                break 'specialize_overlayed_block;
            }
            if elements
                .iter()
                .any(|element| element.blockstate_rotation.is_some())
            {
                break 'specialize_overlayed_block;
            }
            let mut faces = Vec::new();
            for element in elements {
                // TODO The functions for these are already done, just port over from standard block
                // specialization
                if element
                    .faces
                    .iter()
                    .filter_map(|(i, f)| f.map(|f| (i, f)))
                    .any(|(_, face)| face.rotation != RightAngleRotation::Zero)
                {
                    todo!("Support block face texture rotation");
                }
                if element
                    .faces
                    .iter()
                    .filter_map(|(i, f)| f.map(|f| (i, f)))
                    .any(|(_, face)| face.uvs.is_some() && face.uvs != Some([0.0, 0.0, 16.0, 16.0]))
                {
                    todo!("Support block face texture rotation");
                }
                for (index, face) in element.faces.iter() {
                    if let Some(face) = face {
                        faces.push(OverlayedBlockFace {
                            face_i: index as u8,
                            tint: match face.tint_index {
                                -1 => None,
                                // Apparently vanilla only uses one tint index, so anything other
                                // than -1 just means `Tint::Biome`
                                _ => Some(Tint::Biome),
                                // _ => unimplemented!("unknown tint index {}", face.tint_index),
                            },
                            atlas_uvs: texture_atlas
                                .get_or_load_texture(&Identifier::parse(&face.texture)?)?
                                .uvs,
                        });
                    }
                }
            }
            let mut flags = BlockFlags(0);
            flags.set_ambient_occlusion(model_template.ambient_occlusion);
            return Ok(ModelType::OverlayedBlock(OverlayedBlockInfo {
                flags,
                faces,
            }));
        }
        // Fall back to more expensive model rendering if we can't specialize
        let mut converted_vertices: Vec<ModelVertex> = Vec::new();
        let mut converted_indices: Vec<u32> = Vec::new();
        for template_element in model_template.elements.unwrap() {
            const FACE_NORMALS: [Vector3<f32>; 6] = [
                // Top
                Vector3::new(0.0, 1.0, 0.0),
                // Bottom
                Vector3::new(0.0, -1.0, 0.0),
                // North
                Vector3::new(0.0, 0.0, -1.0),
                // South
                Vector3::new(0.0, 0.0, 1.0),
                // East
                Vector3::new(1.0, 0.0, 0.0),
                // West
                Vector3::new(-1.0, 0.0, 0.0),
            ];
            const FACE_INDICES: [u32; 6] = [1, 0, 2, 1, 2, 3];
            const BLOCK_FACES: [BlockFace; 6] = [
                BlockFace::Top,
                BlockFace::Bottom,
                BlockFace::North,
                BlockFace::South,
                BlockFace::East,
                BlockFace::West,
            ];
            let element_faces = [
                template_element.faces.top,
                template_element.faces.bottom,
                template_element.faces.north,
                template_element.faces.south,
                template_element.faces.east,
                template_element.faces.west,
            ];
            for face_i in 0..6 {
                // So I think the issue was using all the PER_FACE_VERTICES.
                // Think we need to swap orders around to fix up front face.
                // Also, are the vertices upside down or something?
                // if face_i != 0 {
                //     continue;
                // }
                let Some(element_face) = element_faces[face_i].clone() else {
                    continue;
                };
                let converted_face = Self::convert_face(
                    texture_atlas,
                    element_face,
                    template_element.blockstate_rotation,
                    BLOCK_FACES[face_i],
                )?;
                /// Converts a Minecraft element coordinate to a model coordinate.
                /// Minecraft element coordinates are 0 to 16 within a block, whereas
                /// we have model coordinates from -0.5 to +0.5.
                fn mc_elem_to_model_coord(point: Point3<f32>) -> Point3<f32> {
                    Point3::from(point.coords.add_scalar(-8.0) / 16.0)
                }
                let start = mc_elem_to_model_coord(template_element.start_pos);
                let end = mc_elem_to_model_coord(template_element.end_pos);
                let face_vertices = match face_i {
                    // Top
                    0 => [
                        Point3::new(start.x, end.y, start.z),
                        Point3::new(end.x, end.y, start.z),
                        Point3::new(start.x, end.y, end.z),
                        Point3::new(end.x, end.y, end.z),
                    ],
                    // Bottom
                    1 => [
                        Point3::new(start.x, start.y, end.z),
                        Point3::new(end.x, start.y, end.z),
                        Point3::new(start.x, start.y, start.z),
                        Point3::new(end.x, start.y, start.z),
                    ],
                    // North
                    2 => [
                        Point3::new(start.x, start.y, start.z),
                        Point3::new(end.x, start.y, start.z),
                        Point3::new(start.x, end.y, start.z),
                        Point3::new(end.x, end.y, start.z),
                    ],
                    // South
                    3 => [
                        Point3::new(end.x, start.y, end.z),
                        Point3::new(start.x, start.y, end.z),
                        Point3::new(end.x, end.y, end.z),
                        Point3::new(start.x, end.y, end.z),
                    ],
                    // East
                    4 => [
                        Point3::new(end.x, start.y, start.z),
                        Point3::new(end.x, start.y, end.z),
                        Point3::new(end.x, end.y, start.z),
                        Point3::new(end.x, end.y, end.z),
                    ],
                    // West
                    5 => [
                        Point3::new(start.x, start.y, end.z),
                        Point3::new(start.x, start.y, start.z),
                        Point3::new(start.x, end.y, end.z),
                        Point3::new(start.x, end.y, start.z),
                    ],
                    _ => unreachable!(),
                };
                // let face_vertices = match face_i {
                //     // Top
                //     0 => [
                //         Vector3::new(-0.5, 0.5, -0.5),
                //         Vector3::new(0.5, 0.5, -0.5),
                //         Vector3::new(-0.5, 0.5, 0.5),
                //         Vector3::new(0.5, 0.5, 0.5),
                //     ],
                //     // Bottom
                //     1 => [
                //         Vector3::new(-0.5, -0.5, 0.5),
                //         Vector3::new(0.5, -0.5, 0.5),
                //         Vector3::new(-0.5, -0.5, -0.5),
                //         Vector3::new(0.5, -0.5, -0.5),
                //     ],
                //     // North
                //     2 => [
                //         Vector3::new(-0.5, -0.5, -0.5),
                //         Vector3::new(0.5, -0.5, -0.5),
                //         Vector3::new(-0.5, 0.5, -0.5),
                //         Vector3::new(0.5, 0.5, -0.5),
                //     ],
                //     // South
                //     3 => [
                //         Vector3::new(0.5, -0.5, 0.5),
                //         Vector3::new(-0.5, -0.5, 0.5),
                //         Vector3::new(0.5, 0.5, 0.5),
                //         Vector3::new(-0.5, 0.5, 0.5),
                //     ],
                //     // East
                //     4 => [
                //         Vector3::new(0.5, -0.5, -0.5),
                //         Vector3::new(0.5, -0.5, 0.5),
                //         Vector3::new(0.5, 0.5, -0.5),
                //         Vector3::new(0.5, 0.5, 0.5),
                //     ],
                //     // West
                //     5 => [
                //         Vector3::new(-0.5, -0.5, 0.5),
                //         Vector3::new(-0.5, -0.5, -0.5),
                //         Vector3::new(-0.5, 0.5, 0.5),
                //         Vector3::new(-0.5, 0.5, -0.5),
                //     ],
                //     _ => unreachable!(),
                // };
                let face_normal = FACE_NORMALS[face_i];
                let num_converted_vertices = u32::try_from(converted_vertices.len()).unwrap();
                let face_indices = FACE_INDICES.map(|index| index + num_converted_vertices);
                let size = end - start;
                let origin = Point3::from((start.coords + end.coords) / 2.0);
                let basic_rotation = match template_element.rotation {
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
                            .prepend_translation(&-origin.coords)
                            .append_translation(&origin.coords);
                        if template_rotation.rescale {
                            let rescale_amount = (1.0 + (1.0 / (angle.cos() - 1.0))) / 2.0;
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
                        }
                    }
                };
                let rotation = match template_element.blockstate_rotation {
                    None => basic_rotation,
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
                                    Rotation3::from_axis_angle(&axis, -std::f32::consts::FRAC_PI_2)
                                        .to_homogeneous()
                                }
                                RightAngleRotation::OneEighty => {
                                    Rotation3::from_axis_angle(&axis, std::f32::consts::PI)
                                        .to_homogeneous()
                                }
                                RightAngleRotation::TwoSeventy => {
                                    Rotation3::from_axis_angle(&axis, std::f32::consts::FRAC_PI_2)
                                        .to_homogeneous()
                                }
                            });
                        y_blockstate_rot * x_blockstate_rot * basic_rotation
                    }
                };
                // let complete_matrix = rotation
                //     .prepend_translation(&origin)
                //     .prepend_nonuniform_scaling(&size);
                let complete_matrix = rotation;
                let mut transformed_vertices = face_vertices.map(|vertex| ModelVertex {
                    local_pos: complete_matrix.transform_point(&vertex),
                    uvs: [0; 2],
                    normal: complete_matrix.transform_vector(&face_normal),
                    tint: converted_face.tint,
                });
                transformed_vertices[0].uvs = [converted_face.uvs[2], converted_face.uvs[3]];
                transformed_vertices[1].uvs = [converted_face.uvs[0], converted_face.uvs[3]];
                transformed_vertices[2].uvs = [converted_face.uvs[2], converted_face.uvs[1]];
                transformed_vertices[3].uvs = [converted_face.uvs[0], converted_face.uvs[1]];
                converted_vertices.extend(transformed_vertices.into_iter());
                converted_indices.extend(face_indices.into_iter());
            }
        }
        Ok(ModelType::Other(OtherInfo {
            vertices: converted_vertices,
            indices: converted_indices,
        }))
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
                    (uv_fractions[2] * x_diff + x_start) as u16,
                    (uv_fractions[3] * y_diff + y_start) as u16,
                ]
            }
        };
        let rotated_uvs = Self::rotate_uvs(custom_uvs, face.rotation);
        Ok(rotated_uvs)
    }

    fn convert_face(
        texture_atlas: &mut texture::AtlasBuilder,
        face: TemplateElementFace,
        blockstate_rotation: Option<TemplateElementBlockstateRotation>,
        index: BlockFace,
    ) -> anyhow::Result<ModelElementFace> {
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
            }) if uv_lock => match index {
                BlockFace::Top => Self::rotate_uvs(transformed_uvs, y_rotation),
                BlockFace::Bottom => Self::rotate_uvs(transformed_uvs, y_rotation),
                BlockFace::East => Self::rotate_uvs(transformed_uvs, x_rotation),
                BlockFace::West => Self::rotate_uvs(transformed_uvs, x_rotation),
                _ => transformed_uvs,
            },
            _ => transformed_uvs,
        };
        Ok(ModelElementFace {
            uvs: rotated_uvs,
            cullface: face.cullface.unwrap_or(index),
            tint: match face.tint_index {
                -1 => None,
                // Apparently vanilla only uses one tint index currently
                _ => Some(Tint::Biome),
                // FIXME: pink_petals_stem uses a tint index of 1?
                // _ => unimplemented!(
                //     "unknown tint index {} when loading {}",
                //     face.tint_index,
                //     face.texture,
                // ),
            },
        })
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
    /// Standard block, has all six faces, only one element, and no fancy stuff. Example: Cobblestone
    Block(BlockInfo),
    /// Tinted block, has all six faces, and only one element. All faces are tinted. Example: Leaves
    TintedBlock(BlockInfo),
    // TODO Make this just have a vec of faces, each face has optional tint index, then make flags
    // just BlockFlags
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

#[derive(Clone, Debug)]
pub struct OverlayedBlockInfo {
    pub flags: BlockFlags,
    pub faces: Vec<OverlayedBlockFace>,
}

#[derive(Clone, Copy, Debug)]
pub struct OverlayedBlockFace {
    pub face_i: u8,
    pub tint: Option<Tint>,
    /// In order of top, bottom, north, south, east and west.
    pub atlas_uvs: [u16; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct CrossInfo {
    pub cross_atlas_start_uvs: [u16; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct LiquidInfo {
    pub uvs: [u16; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OtherInfo {
    pub vertices: Vec<ModelVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct ModelVertex {
    pub local_pos: Point3<f32>,
    pub uvs: [u16; 2],
    pub normal: Vector3<f32>,
    pub tint: Option<Tint>,
}

impl PartialEq for ModelVertex {
    fn eq(&self, other: &Self) -> bool {
        fn f32_eq(x: &f32, y: &f32) -> bool {
            x.total_cmp(y).is_eq()
        }
        fn vec3_eq(left: &Vector3<f32>, right: &Vector3<f32>) -> bool {
            Iterator::zip(left.as_slice().iter(), right.as_slice().iter())
                .map(|(l, r)| f32_eq(l, r))
                .all(std::convert::identity)
        }
        self.uvs == other.uvs
            && self.tint == other.tint
            && vec3_eq(&self.local_pos.coords, &other.local_pos.coords)
            && vec3_eq(&self.normal, &other.normal)
    }
}

impl Eq for ModelVertex {}

impl std::hash::Hash for ModelVertex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        fn f32_hash<H: std::hash::Hasher>(x: &f32, state: &mut H) {
            x.to_bits().hash(state);
        }
        fn vec3_hash<H: std::hash::Hasher>(vec: &Vector3<f32>, state: &mut H) {
            f32_hash(&vec.x, state);
            f32_hash(&vec.y, state);
            f32_hash(&vec.z, state);
        }
        vec3_hash(&self.local_pos.coords, state);
        self.uvs.hash(state);
        vec3_hash(&self.normal, state);
        self.tint.hash(state);
    }
}

#[derive(Clone, Copy, Debug)]
struct ModelElementFace {
    pub uvs: [u16; 4],
    pub cullface: BlockFace,
    pub tint: Option<Tint>,
}

// #[derive(Clone, Debug)]
// pub struct OtherInfo {
//     pub elements: Vec<ModelElement>,
// }

// #[derive(Clone, Copy, Debug)]
// pub struct ModelElement {
//     pub matrix: Matrix4<f32>,
//     pub shade: bool,
//     pub faces: ModelElementFaces,
// }

// #[derive(Clone, Copy, Debug)]
// pub struct ModelElementFaces {
//     pub top: Option<ModelElementFace>,
//     pub bottom: Option<ModelElementFace>,
//     pub north: Option<ModelElementFace>,
//     pub south: Option<ModelElementFace>,
//     pub east: Option<ModelElementFace>,
//     pub west: Option<ModelElementFace>,
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    pub start_pos: Point3<f32>,
    #[serde(rename = "to")]
    pub end_pos: Point3<f32>,
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
    pub origin: Point3<f32>,
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
