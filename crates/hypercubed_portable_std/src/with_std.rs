#![allow(clippy::std_instead_of_alloc)]

pub use std::prelude::rust_2024 as prelude;

pub use std::io;

pub use std::sync;

pub use std::borrow::Cow;
pub use std::collections::{BTreeMap, HashMap, VecDeque};
pub use std::sync::{Arc, Mutex, mpsc};

// TODO: Switch to using `rapidhash`.
pub use ahash::{AHashMap as FastHashMap, AHashSet as FastHashSet};
pub use std::collections::hash_map::Entry as FastHashMapEntry;

pub use string_cache::DefaultAtom as Atom;
