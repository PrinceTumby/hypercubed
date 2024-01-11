pub use string_cache::DefaultAtom as Atom;

use smallvec::SmallVec;
use thiserror::Error;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub namespace: Atom,
    pub path_prefix_segments: SmallVec<[Atom; 2]>,
    pub path_name: Atom,
}

#[macro_export]
macro_rules! identifier {
    ($string:expr) => {
        ($crate::resource::identifier::Identifier::parse($string).unwrap())
    };
}

impl std::fmt::Debug for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "<{}:", &self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}>", &self.path_name)
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}:", &self.namespace)?;
        for path_segment in &self.path_prefix_segments {
            write!(f, "{path_segment}/")?;
        }
        write!(f, "{}", &self.path_name)
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
        if namespace.len() == 0 {
            return false;
        }
        // XXX `for` loops are currently not allowed in const, replace when they get allowed
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
        // XXX `for` loops are currently not allowed in const, replace when they get allowed
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
