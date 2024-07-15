use super::graphics::Camera;
use nalgebra::Vector3;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

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
    // TODO Replace these with mouse movement
    pub look_up: bool,
    pub look_down: bool,
    pub look_left: bool,
    pub look_right: bool,
}

impl PlayControlState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_input(&mut self, event: &KeyEvent) {
        let (state, PhysicalKey::Code(keycode)) = (event.state, event.physical_key) else {
            return;
        };
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
            // Toggle sprint
            KeyCode::ControlLeft if is_pressed => self.sprint = !self.sprint,
            //KeyCode::Space => self.jump = is_pressed,
            KeyCode::BracketRight => self.jump = is_pressed,
            KeyCode::ShiftLeft => self.sneak = is_pressed,
            // TODO Replace these with mouse movement
            KeyCode::ArrowUp => self.look_up = is_pressed,
            KeyCode::ArrowDown => self.look_down = is_pressed,
            KeyCode::ArrowLeft => self.look_left = is_pressed,
            KeyCode::ArrowRight => self.look_right = is_pressed,
            _ => {}
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera, delta_time: f32) {
        let camera_rot = camera.get_rot();
        let speed = if self.sprint { 15.0 } else { 2.0 };
        let forward_dir = camera_rot * *Vector3::z_axis() * delta_time * speed;
        let right_dir = camera_rot * *Vector3::x_axis() * delta_time * speed;
        let up_dir = camera_rot * *Vector3::y_axis() * delta_time * speed;
        const LOOK_SPEED: f32 = 90.0;
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
        if self.look_down {
            camera.pitch -= LOOK_SPEED * delta_time;
        }
        if self.look_up {
            camera.pitch += LOOK_SPEED * delta_time;
        }
        if self.look_left {
            camera.yaw -= LOOK_SPEED * delta_time;
        }
        if self.look_right {
            camera.yaw += LOOK_SPEED * delta_time;
        }
        camera.pitch = camera.pitch.clamp(-90.0, 90.0);
        if camera.yaw < 0.0 {
            camera.yaw += 360.0
        } else if camera.yaw > 360.0 {
            camera.yaw -= 360.0
        }
    }
}
