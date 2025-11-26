// Much of this code is adapted from the `winit` source code.

use super::dpi::{PhysicalPosition, PhysicalSize};
use super::keyboard::PhysicalKey;
use super::window::WindowId;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    NewEvents(StartCause),
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent,
    },
    DeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartCause {
    Init,
    Poll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(usize);

impl DeviceId {
    pub const fn dummy() -> Self {
        Self(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    Resized(PhysicalSize<u32>),
    CloseRequested,
    Destroyed,
    CursorMoved {
        device_id: DeviceId,
        position: PhysicalPosition<f64>,
    },
    CursorLeft {
        device_id: DeviceId,
    },
    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },
    ScaleFactorChanged {
        scale_factor: f64,
        inner_size_writer: (),
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    Added,
    Removed,
    MouseMotion { delta: (f64, f64) },
    Key(RawKeyEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

impl ElementState {
    pub fn is_pressed(self) -> bool {
        self == Self::Pressed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawKeyEvent {
    pub physical_key: PhysicalKey,
    pub state: ElementState,
}
