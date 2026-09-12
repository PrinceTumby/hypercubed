#![allow(clippy::std_instead_of_alloc)]

pub use std::prelude::rust_2024 as prelude;

pub use std::io;

pub use std::sync;

pub use std::borrow::Cow;
pub use std::collections::{BTreeMap, HashMap, VecDeque};
pub use std::sync::{Arc, Mutex, mpsc};

// This currently uses `foldhash` as the hasher, which should be pretty fast.
// TODO: Consider switching to using `rapidhash`.
pub use hashbrown::hash_map::Entry as FastHashMapEntry;
pub use hashbrown::{HashMap as FastHashMap, HashSet as FastHashSet};

pub use string_cache::DefaultAtom as Atom;
