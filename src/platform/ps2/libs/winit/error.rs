#[derive(Debug)]
pub enum ExternalError {
    NotSupported,
    Ignored,
}

impl core::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotSupported => "Not supported".fmt(f),
            Self::Ignored => "Operation was ignored".fmt(f),
        }
    }
}

#[derive(Debug)]
pub enum OsError {}

impl core::fmt::Display for OsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        todo!()
    }
}

impl core::error::Error for OsError {}
