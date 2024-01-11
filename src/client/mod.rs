pub mod graphics;
pub mod input;

use super::resource;
use graphics::GraphicsState;
use input::PlayControlState;
use std::time::Instant;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use crate::identifier;
use resource::block::blockstate;
use resource::block::model::ModelType;
use resource::block::RightAngleRotation;

// fn rotate_uvs([u1, v1, u2, v2]: [u16; 4], rotation: &RightAngleRotation) -> [u16; 4] {
//     match rotation {
//         &RightAngleRotation::Zero => [u1, v1, u2, v2],
//         &RightAngleRotation::Ninety => [v2, u1, u2, v1],
//         &RightAngleRotation::OneEighty => [u2, v2, u1, v1],
//         &RightAngleRotation::TwoSeventy => [u1, v2, u2, v1],
//     }
// }

// TODO Give this a better name
pub async fn window_run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new().build(&event_loop)?;
    window.set_title("Rust Minecraft Client");
    let mut graphics_state =
        GraphicsState::new(&window, super::resource::block::register_vanilla_blocks).await?;
    let mut input_state = PlayControlState::default();
    let mut last_frame_time = Instant::now();
    let cobblestone_info = graphics_state
        .block_registry
        .get_entry_from_identifer(&identifier!("cobblestone"))
        .expect("cobblestone should exist");
    let cobblestone_model_data = &graphics_state.block_registry.global_palette
        [cobblestone_info.default_blockstate.as_usize()]
    .model_data;
    let blockstate::ModelData::Single(cobblestone_model_info) = cobblestone_model_data else {
        unreachable!();
    };
    let cobblestone_model = &cobblestone_model_info.model;
    assert_eq!(cobblestone_model_info.x_rotation, RightAngleRotation::Zero);
    assert_eq!(cobblestone_model_info.y_rotation, RightAngleRotation::Zero);
    let &ModelType::Block(cobblestone) = cobblestone_model.as_ref() else {
        panic!("Cobblestone not a block");
    };
    let grass_block_info = graphics_state
        .block_registry
        .get_entry_from_identifer(&identifier!("grass_block"))
        .expect("grass block should exist");
    let grass_block_model_data = &graphics_state.block_registry.global_palette
        [grass_block_info.default_blockstate.as_usize()]
    .model_data;
    let blockstate::ModelData::RandomChoice(grass_block_models) = grass_block_model_data else {
        unreachable!();
    };
    let grass_block_weighted_model = &grass_block_models[0];
    assert_eq!(cobblestone_model_info.x_rotation, RightAngleRotation::Zero);
    let &ModelType::OverlayedBlock(grass_block) = grass_block_weighted_model.model.as_ref() else {
        panic!("Grass block not an overlayed block");
    };
    let render_block_faces = graphics::chunk::block_face::InstanceList::new(
        &graphics_state.resources.device,
        &[
            graphics::chunk::block_face::Instance {
                pos: [0.0, 0.0, 0.0],
                uvs: cobblestone.per_face_atlas_uvs[0],
                matrix_indices: [0, 0, 0, 0],
            },
            graphics::chunk::block_face::Instance {
                pos: [0.0, 0.0, 0.0],
                uvs: cobblestone.per_face_atlas_uvs[1],
                matrix_indices: [1, 0, 0, 0],
            },
            graphics::chunk::block_face::Instance {
                pos: [0.0, 0.0, 0.0],
                uvs: cobblestone.per_face_atlas_uvs[2],
                matrix_indices: [2, 0, 0, 0],
            },
            graphics::chunk::block_face::Instance {
                pos: [0.0, 0.0, 0.0],
                uvs: cobblestone.per_face_atlas_uvs[3],
                matrix_indices: [3, 0, 0, 0],
            },
            graphics::chunk::block_face::Instance {
                pos: [0.0, 0.0, 0.0],
                uvs: cobblestone.per_face_atlas_uvs[5],
                matrix_indices: [5, 0, 0, 0],
            },
            graphics::chunk::block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_base_face_atlas_uvs[1],
                matrix_indices: [
                    1,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
            },
            graphics::chunk::block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_base_face_atlas_uvs[2],
                matrix_indices: [
                    2,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
            },
            graphics::chunk::block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_base_face_atlas_uvs[3],
                matrix_indices: [
                    3,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
            },
            graphics::chunk::block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_base_face_atlas_uvs[4],
                matrix_indices: [
                    4,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
            },
        ],
    );
    let render_tinted_block_faces = graphics::chunk::tinted_block_face::InstanceList::new(
        &graphics_state.resources.device,
        &[
            graphics::chunk::tinted_block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_base_face_atlas_uvs[0],
                matrix_indices: [
                    0,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
                tint_color: [0x91, 0xBD, 0x59, 0xFF],
            },
            graphics::chunk::tinted_block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_overlay_face_atlas_uvs[1],
                matrix_indices: [
                    1,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
                tint_color: [0x91, 0xBD, 0x59, 0xFF],
            },
            graphics::chunk::tinted_block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_overlay_face_atlas_uvs[2],
                matrix_indices: [
                    2,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
                tint_color: [0x91, 0xBD, 0x59, 0xFF],
            },
            graphics::chunk::tinted_block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_overlay_face_atlas_uvs[3],
                matrix_indices: [
                    3,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
                tint_color: [0x91, 0xBD, 0x59, 0xFF],
            },
            graphics::chunk::tinted_block_face::Instance {
                pos: [1.0, 0.0, 0.0],
                uvs: grass_block.per_overlay_face_atlas_uvs[4],
                matrix_indices: [
                    4,
                    0,
                    grass_block_weighted_model.y_rotation.matrix_index(),
                    0,
                ],
                tint_color: [0x91, 0xBD, 0x59, 0xFF],
            },
        ],
    );
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::NewEvents(StartCause::Poll) => {
                let new_time = Instant::now();
                let delta_time =
                    (new_time - std::mem::replace(&mut last_frame_time, new_time)).as_secs_f32();
                input_state.update_camera(&mut graphics_state.camera, delta_time);
                match graphics_state.render(&render_block_faces, &render_tinted_block_faces) {
                    Ok(()) | Err(wgpu::SurfaceError::Timeout) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => {
                        let size = graphics_state.size;
                        graphics_state.resize(size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => window_target.exit(),
                    // All other errors (Timeout, etc) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested | WindowEvent::Destroyed => window_target.exit(),
                WindowEvent::Resized(physical_size) => graphics_state.resize(*physical_size),
                WindowEvent::KeyboardInput {
                    device_id: _,
                    event,
                    is_synthetic,
                } if !is_synthetic => input_state.update_from_input(event),
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}
