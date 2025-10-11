use bitfield::bitfield;

bitfield! {
    // 0: Ignore depth?
    // 1-31: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, Default)]
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

pub mod point {
    use super::*;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub color: [u8; 4],
        pub size: f32,
        pub packed_fields: PackedFlags,
    }
}

pub mod line {
    use super::*;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct Instance {
        pub p1: [f32; 3],
        pub p2: [f32; 3],
        pub color: [u8; 4],
        pub packed_fields: PackedFlags,
    }
}

pub mod triangle {
    use super::*;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct Instance {
        pub p1: [f32; 3],
        pub p2: [f32; 3],
        pub p3: [f32; 3],
        pub color: [u8; 4],
        pub packed_fields: PackedFlags,
    }
}
