pub use portable_std::prelude::*;
pub use portable_std::Atom;

use smallvec::SmallVec;
use thiserror::Error;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier {
    pub namespace: Atom,
    pub path_prefix_segments: SmallVec<[Atom; 2]>,
    pub path_name: Atom,
}

#[macro_export]
macro_rules! identifier {
    ($string:expr) => {
        ($crate::identifier::Identifier::parse($string).unwrap())
    };
}

impl core::fmt::Debug for Identifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "<{}:", &self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}>", &self.path_name)
    }
}

impl core::fmt::Display for Identifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "{}:", &self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}", &self.path_name)
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

#[derive(Clone, Copy, Debug, Error)]
pub enum ParseIdentifierError {
    #[error("invalid namespace")]
    InvalidNamespace,
    #[error("invalid path")]
    InvalidPath,
}

impl Identifier {
    pub fn parse(str_identifier: &str) -> Result<Self, ParseIdentifierError> {
        let (namespace, path_str) = match str_identifier.find(':') {
            None => (Atom::from("minecraft"), str_identifier),
            Some(colon_position) => {
                let namespace_str = &str_identifier[..colon_position];
                if !Self::is_valid_namespace(namespace_str) {
                    return Err(ParseIdentifierError::InvalidNamespace);
                }
                (
                    Atom::from(namespace_str),
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
                path_name: Atom::from(path_str),
            }),
            Some(final_slash_position) => {
                let mut path_prefix_segments = SmallVec::new_const();
                for segment in path_str[..final_slash_position].split('/') {
                    path_prefix_segments.push(Atom::from(segment));
                }
                Ok(Self {
                    namespace,
                    path_prefix_segments,
                    path_name: Atom::from(&path_str[final_slash_position + 1..]),
                })
            }
        }
    }

    const fn is_valid_namespace(namespace: &str) -> bool {
        if namespace.is_empty() {
            return false;
        }
        // XXX: `for` loops are currently not allowed in const, replace when they get allowed
        let namespace_bytes = namespace.as_bytes();
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

    const fn is_valid_path(namespace: &str) -> bool {
        // XXX: `for` loops are currently not allowed in const, replace when they get allowed
        let namespace_bytes = namespace.as_bytes();
        #[derive(Clone, Copy)]
        enum State {
            AnyPathCharacter,
            NotForwardSlash,
        }
        let mut state = State::NotForwardSlash;
        let mut i: usize = 0;
        while i < namespace_bytes.len() {
            let character = namespace_bytes[i];
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
