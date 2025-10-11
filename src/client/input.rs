use super::graphics::Camera;
use nalgebra::Vector3;

#[cfg(feature = "full_std")]
use std_imports::*;
#[cfg(feature = "full_std")]
mod std_imports {
    pub use winit::event::ElementState;
    pub use winit::keyboard::{KeyCode, PhysicalKey};
}

#[cfg(not(feature = "full_std"))]
use no_std_imports::*;
#[cfg(not(feature = "full_std"))]
mod no_std_imports {
    pub use crate::platform::libs::winit;
    pub use winit::event::ElementState;
    pub use winit::keyboard::{KeyCode, PhysicalKey};
}

// TODO We probably want to convert this to an action system at some point, where actions are
// registered as being continuous, single press, toggle, etc. Would make modded custom controls
// much easier.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayControlState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    // Used as a flag for whether sprinting should be continually retried. Is enabled if sprint
    // toggling is off and Left Control is held down.
    pub trying_to_sprint: bool,
    // TODO: Rename this to better reflect that it enables all gameplay input, not just mouse
    // movement.
    pub mouse_locked: bool,
    pub fullscreen: bool,
}

impl PlayControlState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_input(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
        sprint_toggling: bool,
    ) {
        let (state, PhysicalKey::Code(keycode)) = (state, physical_key) else {
            return;
        };
        if !self.mouse_locked && !matches!(keycode, KeyCode::Escape | KeyCode::KeyF) {
            return;
        }
        // Ensures a compile error if additional states get added to `ElementState`
        let is_pressed = match state {
            ElementState::Pressed => true,
            ElementState::Released => false,
        };
        match keycode {
            KeyCode::KeyW => self.forward = is_pressed,
            KeyCode::KeyS => self.backward = is_pressed,
            KeyCode::KeyA => self.left = is_pressed,
            KeyCode::KeyD => self.right = is_pressed,
            // Toggle or enable sprint
            KeyCode::ControlLeft => {
                if sprint_toggling {
                    if is_pressed {
                        self.trying_to_sprint = false;
                        self.sprint = !self.sprint;
                    }
                } else {
                    self.trying_to_sprint = is_pressed;
                    if is_pressed {
                        self.sprint = true;
                    }
                }
            }
            //KeyCode::Space => self.jump = is_pressed,
            KeyCode::BracketRight => self.jump = is_pressed,
            KeyCode::ShiftLeft => self.sneak = is_pressed,
            KeyCode::Escape if is_pressed => {
                self.mouse_locked = !self.mouse_locked;
                // Turn off all input if we've just unlocked game inputs, so that we don't have
                // stuck inputs.
                if !self.mouse_locked {
                    *self = Self {
                        mouse_locked: self.mouse_locked,
                        fullscreen: self.fullscreen,
                        ..Default::default()
                    };
                }
            }
            KeyCode::KeyF if is_pressed => self.fullscreen = !self.fullscreen,
            _ => {}
        }
    }

    pub fn update_fly_camera_pos(&self, camera: &mut Camera, delta_time: f32) {
        let camera_rot = camera.get_rot();
        let speed = if self.sprint { 15.0 } else { 2.0 };
        let forward_dir = camera_rot * *Vector3::z_axis() * delta_time * speed;
        let right_dir = camera_rot * *Vector3::x_axis() * delta_time * speed;
        let up_dir = camera_rot * *Vector3::y_axis() * delta_time * speed;
        if self.forward {
            camera.pos -= forward_dir;
        }
        if self.backward {
            camera.pos += forward_dir;
        }
        if self.left {
            camera.pos -= right_dir;
        }
        if self.right {
            camera.pos += right_dir;
        }
        if self.sneak {
            camera.pos -= up_dir;
        }
        if self.jump {
            camera.pos += up_dir;
        }
    }
}
