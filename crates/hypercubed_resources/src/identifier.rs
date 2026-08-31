pub use portable_std::Atom;
pub use portable_std::prelude::*;

use smallvec::SmallVec;

/// Convenience macro for creating an [`Identifier`] from a string literal.
#[macro_export]
macro_rules! identifier {
    ($string:literal) => {
        ($crate::identifier::Identifier::parse($string).unwrap())
    };
}

/// Convenience macro for creating an [`IdentifierPart`] from a string literal.
#[macro_export]
macro_rules! identifier_part {
    ($string:literal) => {
        ($crate::identifier::IdentifierPart::parse($string).unwrap())
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid identifier part")]
pub struct ParseIdentifierPartError;

/// A valid part of an [`Identifier`], specified by the regex `/[a-z0-9_\-.]/`.
///
/// This is interned, so [`Clone`] is cheap.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct IdentifierPart(Atom);

impl core::ops::Deref for IdentifierPart {
    type Target = Atom;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::fmt::Debug for IdentifierPart {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

impl core::fmt::Display for IdentifierPart {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<&str> for IdentifierPart {
    type Error = ParseIdentifierPartError;

    fn try_from(str_part: &str) -> Result<Self, ParseIdentifierPartError> {
        Self::parse(str_part)
    }
}

impl IdentifierPart {
    pub fn parse(str_part: &str) -> Result<Self, ParseIdentifierPartError> {
        if !Self::is_valid_from(str_part) {
            return Err(ParseIdentifierPartError);
        }
        Ok(Self(Atom::from(str_part)))
    }

    /// Returns whether `str_part` would be a valid identifier part.
    pub const fn is_valid_from(str_part: &str) -> bool {
        if str_part.is_empty() {
            return false;
        }
        let part_bytes = str_part.as_bytes();
        // XXX: `for` loops are currently not allowed in const, replace when they get allowed.
        let mut i: usize = 0;
        while i < part_bytes.len() {
            let byte = part_bytes[i];
            if !matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.') {
                return false;
            }
            i += 1;
        }
        true
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum ParseIdentifierError {
    #[error("invalid namespace")]
    InvalidNamespace,
    #[error("invalid path")]
    InvalidPath,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier {
    namespace: IdentifierPart,
    path_prefix_segments: SmallVec<[IdentifierPart; 2]>,
    path_name: IdentifierPart,
}

impl core::fmt::Debug for Identifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "<{}:", self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}>", self.path_name)
    }
}

impl core::fmt::Display for Identifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "{}:", self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}", self.path_name)
    }
}

impl TryFrom<&str> for Identifier {
    type Error = ParseIdentifierError;

    fn try_from(value: &str) -> Result<Self, ParseIdentifierError> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Identifier {
    type Error = ParseIdentifierError;

    fn try_from(value: String) -> Result<Self, ParseIdentifierError> {
        Self::parse(&value)
    }
}

impl From<Identifier> for String {
    fn from(identifier: Identifier) -> String {
        use core::fmt::Write;
        let mut out_string = String::new();
        write!(out_string, "{identifier}").unwrap();
        out_string
    }
}

impl Identifier {
    pub const DEFAULT_NAMESPACE_STR: &str = "minecraft";

    pub fn parse(str_identifier: &str) -> Result<Self, ParseIdentifierError> {
        let (namespace, path_str) = match str_identifier.find(':') {
            None => (IdentifierPart::parse(Self::DEFAULT_NAMESPACE_STR).unwrap(), str_identifier),
            Some(colon_position) => {
                let namespace_str = &str_identifier[..colon_position];
                if !Self::is_valid_namespace(namespace_str) {
                    return Err(ParseIdentifierError::InvalidNamespace);
                }
                (
                    IdentifierPart::parse(namespace_str).unwrap(),
                    &str_identifier[colon_position + 1..],
                )
            }
        };
        if !Self::is_valid_path(path_str) {
            return Err(ParseIdentifierError::InvalidPath);
        }
        match path_str.rfind('/') {
            None => Ok(Self {
                namespace,
                path_prefix_segments: SmallVec::new_const(),
                path_name: IdentifierPart::parse(path_str).unwrap(),
            }),
            Some(final_slash_position) => {
                let mut path_prefix_segments = SmallVec::new_const();
                for segment in path_str[..final_slash_position].split('/') {
                    path_prefix_segments.push(IdentifierPart::parse(segment).unwrap());
                }
                Ok(Self {
                    namespace,
                    path_prefix_segments,
                    path_name: IdentifierPart::parse(&path_str[final_slash_position + 1..]).unwrap(),
                })
            }
        }
    }

    pub fn from_parts(
        namespace: Option<IdentifierPart>,
        path_prefix_segments: impl IntoIterator<Item = IdentifierPart>,
        path_name: IdentifierPart,
    ) -> Self {
        Self {
            namespace: namespace.unwrap_or_else(|| IdentifierPart::parse(Self::DEFAULT_NAMESPACE_STR).unwrap()),
            path_prefix_segments: path_prefix_segments.into_iter().collect(),
            path_name,
        }
    }

    pub fn get_namespace(&self) -> &IdentifierPart {
        &self.namespace
    }

    pub fn get_path_prefix_segments(&self) -> &[IdentifierPart] {
        &self.path_prefix_segments
    }

    pub fn get_path_name(&self) -> &IdentifierPart {
        &self.path_name
    }

    const fn is_valid_namespace(namespace: &str) -> bool {
        if namespace.is_empty() {
            return false;
        }
        let namespace_bytes = namespace.as_bytes();
        // XXX: `for` loops are currently not allowed in const, replace when they get allowed.
        let mut i: usize = 0;
        while i < namespace_bytes.len() {
            let character = namespace_bytes[i];
            match character {
                b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' => {}
                _ => return false,
            }
            i += 1;
        }
        true
    }

    const fn is_valid_path(path: &str) -> bool {
        let path_bytes = path.as_bytes();
        #[derive(Clone, Copy)]
        enum State {
            AnyPathCharacter,
            NotForwardSlash,
        }
        let mut state = State::NotForwardSlash;
        // XXX: `for` loops are currently not allowed in const, replace when they get allowed.
        let mut i: usize = 0;
        while i < path_bytes.len() {
            let character = path_bytes[i];
            match (state, character) {
                (State::AnyPathCharacter, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.') => {}
                (State::AnyPathCharacter, b'/') => state = State::NotForwardSlash,
                (State::NotForwardSlash, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.') => {
                    state = State::AnyPathCharacter;
                }
                _ => return false,
            }
            i += 1;
        }
        if matches!(state, State::NotForwardSlash) {
            return false;
        }
        true
    }
}
