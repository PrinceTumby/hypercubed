pub mod block;
pub mod identifier;
pub mod manager;
pub mod texture;

pub use identifier::{Atom as IdentifierAtom, Identifier, ParseIdentifierError};

use bimap::hash::BiHashMap;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegistryIndex(u16);

#[derive(Default)]
struct RegistryData<T> {
    entries: Vec<T>,
    identifier_map: BiHashMap<Identifier, RegistryIndex>,
}

impl<T> RegistryData<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            identifier_map: BiHashMap::new(),
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

impl<T> std::ops::Index<RegistryIndex> for RegistryData<T> {
    type Output = T;

    fn index(&self, index: RegistryIndex) -> &Self::Output {
        &self.entries[index.0 as usize]
    }
}

impl<T> std::ops::IndexMut<RegistryIndex> for RegistryData<T> {
    fn index_mut(&mut self, index: RegistryIndex) -> &mut Self::Output {
        &mut self.entries[index.0 as usize]
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for RegistryData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.identifier_map
                    .iter()
                    .map(|(ident, idx)| (ident, &self.entries[idx.0 as usize])),
            )
            .finish()
    }
}
