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

impl<P> From<(P, P)> for PhysicalPosition<P> {
    fn from((x, y): (P, P)) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalSize<P> {
    pub width: P,
    pub height: P,
}

impl<P> From<(P, P)> for PhysicalSize<P> {
    fn from((width, height): (P, P)) -> Self {
        Self { width, height }
    }
}
