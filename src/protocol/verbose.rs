use crate::prelude::Vec;
use nom::ErrorConvert;
use nom::error::{ContextError, ErrorKind, FromExternalError, ParseError};

pub type VerboseError<'a> = GenericVerboseError<super::prelude::InputSpan<'a>>;
pub type ByteViewVerboseError<'a> = GenericVerboseError<super::ByteView<'a>>;

fn convert_error(error: VerboseError) -> ByteViewVerboseError {
    ByteViewVerboseError {
        errors: error
            .errors
            .into_iter()
            .map(|(location, stack_context)| (location.into(), stack_context))
            .collect(),
    }
}

// This has mostly been based on nom_language's VerboseError type.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericVerboseError<I> {
    pub errors: Vec<(I, VerboseErrorKind)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Error context for `VerboseError`
pub enum VerboseErrorKind {
    /// Static string added by the `context` function
    Context(&'static str),
    /// Indicates which character was expected by the `char` function
    Char(char),
    /// Error kind given by various nom parsers
    Nom(ErrorKind),
}

impl<I> ParseError<I> for GenericVerboseError<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        GenericVerboseError {
            errors: vec![(input, VerboseErrorKind::Nom(kind))],
        }
    }

    fn append(input: I, kind: ErrorKind, mut other: Self) -> Self {
        other.errors.push((input, VerboseErrorKind::Nom(kind)));
        other
    }

    fn from_char(input: I, c: char) -> Self {
        GenericVerboseError {
            errors: vec![(input, VerboseErrorKind::Char(c))],
        }
    }
}

impl<I> ContextError<I> for GenericVerboseError<I> {
    fn add_context(input: I, ctx: &'static str, mut other: Self) -> Self {
        other.errors.push((input, VerboseErrorKind::Context(ctx)));
        other
    }
}

impl<I, E> FromExternalError<I, E> for GenericVerboseError<I> {
    /// Create a new error from an input position and an external error
    fn from_external_error(input: I, kind: ErrorKind, _e: E) -> Self {
        Self::from_error_kind(input, kind)
    }
}

impl<I: std::fmt::Display> std::fmt::Display for GenericVerboseError<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Parse error:")?;
        for (input, error) in &self.errors {
            match error {
                VerboseErrorKind::Nom(e) => writeln!(f, "{:?} at: {}", e, input)?,
                VerboseErrorKind::Char(c) => writeln!(f, "expected '{}' at: {}", c, input)?,
                VerboseErrorKind::Context(s) => writeln!(f, "in section '{}', at: {}", s, input)?,
            }
        }

        Ok(())
    }
}

impl<I: std::fmt::Debug + std::fmt::Display> std::error::Error for GenericVerboseError<I> {}

impl From<GenericVerboseError<&[u8]>> for GenericVerboseError<Vec<u8>> {
    fn from(value: GenericVerboseError<&[u8]>) -> Self {
        GenericVerboseError {
            errors: value
                .errors
                .into_iter()
                .map(|(i, e)| (i.to_owned(), e))
                .collect(),
        }
    }
}

impl<I> ErrorConvert<GenericVerboseError<I>> for GenericVerboseError<(I, usize)> {
    fn convert(self) -> GenericVerboseError<I> {
        GenericVerboseError {
            errors: self.errors.into_iter().map(|(i, e)| (i.0, e)).collect(),
        }
    }
}

impl<I> ErrorConvert<GenericVerboseError<(I, usize)>> for GenericVerboseError<I> {
    fn convert(self) -> GenericVerboseError<(I, usize)> {
        GenericVerboseError {
            errors: self.errors.into_iter().map(|(i, e)| ((i, 0), e)).collect(),
        }
    }
}
