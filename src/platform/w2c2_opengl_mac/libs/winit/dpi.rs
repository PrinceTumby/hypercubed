// Much of this code is adapted from the `winit` source code.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalPosition<P> {
    pub x: P,
    pub y: P,
}

impl<P> PhysicalPosition<P> {
    pub const fn new(x: P, y: P) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalSize<P> {
    pub width: P,
    pub height: P,
}
