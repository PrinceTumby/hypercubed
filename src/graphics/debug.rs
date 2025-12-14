use bitfield::bitfield;

bitfield! {
    // 0: Ignore depth?
    // 1-31: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PackedFlags(u32);
    impl Debug;
    pub ignore_depth, set_ignore_depth: 0;
}

impl PackedFlags {
    pub const NONE: Self = Self(0);
    pub const IGNORE_DEPTH: Self = Self(1);

    pub fn new(ignore_depth: bool) -> Self {
        let mut fields = Self::NONE;
        fields.set_ignore_depth(ignore_depth);
        fields
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Point {
    pub pos: [f32; 3],
    pub size: f32,
    pub color: [u8; 4],
    pub flags: PackedFlags,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Line {
    pub p1: [f32; 3],
    pub p2: [f32; 3],
    pub size: f32,
    pub colour: [u8; 4],
    pub flags: PackedFlags,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Triangle {
    pub p1: [f32; 3],
    pub p2: [f32; 3],
    pub p3: [f32; 3],
    pub color: [u8; 4],
    pub flags: PackedFlags,
}
