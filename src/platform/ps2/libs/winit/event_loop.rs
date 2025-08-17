// Much of this code is adapted from the `winit` source code.

use super::event::Event;
use super::window::Window;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    #[default]
    Wait,
}

#[derive(Debug)]
pub struct EventLoop {}

impl EventLoop {
    pub fn new() -> Result<Self, EventLoopError> {
        Ok(Self {})
    }

    pub fn exit(&self) {
        todo!("EventLoop::exit")
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        assert!(control_flow == ControlFlow::Poll)
    }

    pub fn run<F>(self, event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event, &Self),
    {
        todo!("EventLoop::run")
    }
}

#[derive(Debug)]
pub enum EventLoopError {}

impl core::fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        todo!()
    }
}

impl core::error::Error for EventLoopError {}
