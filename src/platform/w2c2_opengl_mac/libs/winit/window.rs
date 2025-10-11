// Much of this code is adapted from the `winit` source code.

use super::dpi::{PhysicalPosition, PhysicalSize};
use super::error::{ExternalError, OsError};
use super::event_loop::EventLoop;

#[derive(Clone, Debug, Default)]
pub struct WindowBuilder {}

impl WindowBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self, _event_loop: &EventLoop) -> Result<Window, OsError> {
        Ok(Window {})
    }
}

#[derive(Debug)]
pub struct Window {}

impl Window {
    pub fn has_focus(&self) -> bool {
        true
    }

    pub fn id(&self) -> WindowId {
        WindowId(())
    }

    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    pub fn set_cursor_grab(&self, _mode: CursorGrabMode) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported)
    }

    pub fn set_cursor_position(
        &self,
        _position: PhysicalPosition<i32>,
    ) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported)
    }

    pub fn set_cursor_visible(&self, _visible: bool) {}

    pub fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) {}

    pub fn set_title(&self, _title: &str) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub ());

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Borderless(Option<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorGrabMode {
    None,
    Confined,
    Locked,
}
