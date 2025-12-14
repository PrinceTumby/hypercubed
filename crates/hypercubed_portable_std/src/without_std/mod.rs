pub mod prelude {
    pub use super::{Box, String, ToOwned, ToString, Vec};
    pub use nalgebra::{ComplexField, RealField};
}

pub mod io;

pub mod sync;

pub use alloc::borrow::{Cow, ToOwned};
pub use alloc::boxed::Box;
pub use alloc::collections::{BTreeMap, VecDeque};
pub use alloc::string::{String, ToString};
pub use alloc::vec::Vec;
pub use sync::Arc;

#[macro_export]
macro_rules! format {
    ($($arg:tt)*) => { ::alloc::format!($($arg:tt)*) };
}

#[macro_export]
macro_rules! vec {
    () => { ::alloc::vec![] };
    ($elem:expr; $n:expr) => { ::alloc::vec![$elem; $n] };
    ($($x:expr),+ $(,)?) => { ::alloc::vec![$($x),+] };
}

pub use hashbrown::HashMap;
// This currently uses `foldhash` as the hasher, which should be pretty fast.
pub use hashbrown::hash_map::Entry as FastHashMapEntry;
pub use hashbrown::{HashMap as FastHashMap, HashSet as FastHashSet};

// TODO: For performance, we probably want to write a no_std version of string_cache.
// - Design could be a `no_hash::IntMap` on string length, containing HashMaps of `Arc<str>`.
// - Should reduce inner map reallocation frequency, while keeping fast interning.
pub type Atom = smallstr::SmallString<[u8; 12]>;
