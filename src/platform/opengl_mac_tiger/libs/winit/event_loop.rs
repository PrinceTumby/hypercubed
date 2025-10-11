// Much of this code is adapted from the `winit` source code.

use super::dpi::PhysicalSize;
use super::event::{
    DeviceEvent, DeviceId, ElementState, Event, RawKeyEvent, StartCause, WindowEvent,
};
use super::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
use super::window::WindowId;

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    #[default]
    Wait,
}

#[derive(Debug)]
pub struct EventLoop {
    close_next_poll: AtomicBool,
}

impl EventLoop {
    pub fn new() -> Result<Self, EventLoopError> {
        Ok(Self {
            close_next_poll: AtomicBool::new(false),
        })
    }

    pub fn exit(&self) {
        self.close_next_poll.store(true, Ordering::SeqCst);
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        assert!(control_flow == ControlFlow::Poll)
    }

    pub fn run<F>(self, mut event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event, &Self),
    {
        todo!("EventLoop::run")
    }
}

#[derive(Debug)]
pub enum EventLoopError {}

impl core::fmt::Display for EventLoopError {
    fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        todo!()
    }
}

impl core::error::Error for EventLoopError {}
