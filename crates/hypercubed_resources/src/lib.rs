#![cfg_attr(not(feature = "std"), no_std, allow(unused))]

pub mod aabb;
pub mod block;
pub mod environment;
pub mod identifier;
#[cfg(feature = "std")]
pub mod manager;
pub mod texture;

pub use identifier::{Atom as IdentifierAtom, Identifier, ParseIdentifierError};

use anyhow::Context;
use bimap::BiMap;
use portable_std::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GameResourceData {
    pub block_data: block::ResourceData,
    pub environment_data: environment::ResourceData,
}

#[cfg(feature = "std")]
impl GameResourceData {
    /// Loads the vanilla game data from either a vanilla client `minecraft.jar` file, or from an
    /// extracted `assets`.
    /// Can be used either to load the game data at run time, or from a build script to then embed
    /// the data at compile time.
    #[cfg(feature = "std")]
    pub fn load_vanilla_data() -> anyhow::Result<GameResourceData> {
        let block_data = block::ResourceData::load_vanilla_data()
            .context("Error while loading block resource data")?;
        let environment_data = environment::ResourceData::load_vanilla_data()
            .context("Error while loading environment resource data")?;
        Ok(Self {
            block_data,
            environment_data,
        })
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegistryIndex(u16);

#[derive(Default, Serialize, Deserialize)]
struct RegistryData<T> {
    entries: Vec<T>,
    identifier_map: BiMap<Identifier, RegistryIndex>,
}

impl<T> RegistryData<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            identifier_map: BiMap::new(),
        }
    }

    /// Panics if an entry is already registered with `identifier`.
    pub fn register(&mut self, identifier: Identifier, entry: T) -> RegistryIndex {
        let index = RegistryIndex(self.entries.len().try_into().unwrap());
        if let Err((identifier, _index)) =
            self.identifier_map.insert_no_overwrite(identifier, index)
        {
            panic!("registry already contains key {identifier}");
        }
        self.entries.push(entry);
        index
    }

    pub fn get_index_from_identifier(&self, identifier: &Identifier) -> Option<RegistryIndex> {
        self.identifier_map.get_by_left(identifier).copied()
    }

    pub fn get_entry_from_identifier(&self, identifier: &Identifier) -> Option<&T> {
        let index = self.get_index_from_identifier(identifier)?;
        Some(&self.entries[index.0 as usize])
    }

    pub fn get_identifier_from_index(&self, index: RegistryIndex) -> Option<&Identifier> {
        self.identifier_map.get_by_right(&index)
    }
}

impl<T: Default> RegistryData<T> {
    /// Panics if an entry is already registered with `identifier`.
    pub fn register_default(&mut self, identifier: Identifier) -> RegistryIndex {
        self.register(identifier, T::default())
    }
}

impl<T> core::ops::Index<RegistryIndex> for RegistryData<T> {
    type Output = T;

    fn index(&self, index: RegistryIndex) -> &Self::Output {
        &self.entries[index.0 as usize]
    }
}

impl<T> core::ops::IndexMut<RegistryIndex> for RegistryData<T> {
    fn index_mut(&mut self, index: RegistryIndex) -> &mut Self::Output {
        &mut self.entries[index.0 as usize]
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for RegistryData<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map()
            .entries(
                self.identifier_map
                    .iter()
                    .map(|(ident, idx)| (ident, &self.entries[idx.0 as usize])),
            )
            .finish()
    }
}
