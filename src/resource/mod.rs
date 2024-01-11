pub mod block;
pub mod identifier;
pub mod manager;
pub mod texture;

pub use identifier::{Atom as IdentifierAtom, Identifier, ParseIdentifierError};

use ahash::AHashMap;

struct RegistryData<T: std::fmt::Debug> {
    entries: Vec<T>,
    identifier_map: AHashMap<Identifier, RegistryIndex>,
}

impl<T: std::fmt::Debug> RegistryData<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            identifier_map: AHashMap::new(),
        }
    }

    /// Panics if an entry is already registered with `identifier`.
    pub fn register(&mut self, identifier: Identifier, entry: T) -> RegistryIndex {
        let index = RegistryIndex(self.entries.len().try_into().unwrap());
        assert!(
            self.identifier_map
                .insert(identifier.clone(), index)
                .is_none(),
            "registry already contains key {}",
            identifier,
        );
        self.entries.push(entry);
        index
    }

    pub fn get_entry_from_identifer(&self, identifier: &Identifier) -> Option<&T> {
        let index = self.identifier_map.get(identifier)?;
        Some(&self.entries[index.0 as usize])
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

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryIndex(u16);
