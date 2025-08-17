pub mod prelude {
    pub use super::{Read, Write /*, Seek*/};
}

pub use embedded_io::{ErrorKind, ReadExactError};

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    fn read_to_end(&mut self, buf: &mut super::Vec<u8>) -> Result<usize> {
        let mut intermediate_buffer = [0u8; 32];
        let start_len = buf.len();
        loop {
            match self.read(&mut intermediate_buffer) {
                Ok(0) => return Ok(buf.len() - start_len),
                Ok(bytes_read) => {
                    buf.extend_from_slice(&intermediate_buffer[0..bytes_read])
                }
                Err(err) if err.is_interrupted() => continue,
                Err(err) => return Err(err),
            }
        }
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> core::result::Result<(), ReadExactError<Error>> {
        let mut buf = buf;
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(bytes_read) => buf = &mut buf[bytes_read..],
                Err(ref err) if err.is_interrupted() => continue,
                Err(err) => return Err(ReadExactError::Other(err)),
            }
        }
        if !buf.is_empty() {
            Err(ReadExactError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

impl<R: Read + ?Sized> Read for &mut R {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }

    fn read_to_end(&mut self, buf: &mut super::Vec<u8>) -> Result<usize> {
        (**self).read_to_end(buf)
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> core::result::Result<(), ReadExactError<Error>> {
        (**self).read_exact(buf)
    }
}

impl<R: Read + ?Sized> Read for super::Box<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }

    fn read_to_end(&mut self, buf: &mut super::Vec<u8>) -> Result<usize> {
        (**self).read_to_end(buf)
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> core::result::Result<(), ReadExactError<Error>> {
        (**self).read_exact(buf)
    }
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    fn flush(&mut self) -> Result<()>;

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        let mut buf = buf;
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(Error(ErrorRepr::Simple(ErrorKind::WriteZero))),
                Ok(bytes_written) => buf = &buf[bytes_written..],
                Err(ref err) if err.is_interrupted() => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
        if let Some(s) = args.as_str() {
            self.write_all(s.as_bytes())
        } else {
            struct Adaptor<'a, T: Write + ?Sized + 'a> {
                inner: &'a mut T,
                error: Option<Error>,
            }

            impl<T: Write + ?Sized> core::fmt::Write for Adaptor<'_, T> {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    match self.inner.write_all(s.as_bytes()) {
                        Ok(()) => Ok(()),
                        Err(err) => {
                            self.error = Some(err);
                            Err(core::fmt::Error)
                        }
                    }
                }
            }

            let mut adaptor = Adaptor {
                inner: self,
                error: None,
            };
            match core::fmt::Write::write_fmt(&mut adaptor, args) {
                Ok(()) => Ok(()),
                Err(core::fmt::Error) => match adaptor.error {
                    Some(err) => Err(err),
                    None => Err(Error(ErrorRepr::Simple(ErrorKind::Other))),
                },
            }
        }
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

impl Write for &mut [u8] {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let num_bytes_to_write = usize::min(self.len(), buf.len());
        let (a, rest) = core::mem::take(self).split_at_mut(num_bytes_to_write);
        a.copy_from_slice(&buf[0..num_bytes_to_write]);
        *self = rest;
        Ok(num_bytes_to_write)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Write for super::Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<W: Write + ?Sized> Write for &mut W {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (**self).write_all(buf)
    }

    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
        (**self).write_fmt(args)
    }
}

impl<W: Write + ?Sized> Write for super::Box<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (**self).write_all(buf)
    }

    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<()> {
        (**self).write_fmt(args)
    }
}

// TODO: Seek

pub type Result<T> = core::result::Result<T, Error>;

pub struct Error(ErrorRepr);

enum ErrorRepr {
    Simple(ErrorKind),
    Custom(CustomError),
}

#[derive(Debug)]
struct CustomError {
    pub kind: ErrorKind,
    pub error: super::Box<dyn core::error::Error + Send + Sync>,
}

impl Error {
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<super::Box<dyn core::error::Error + Send + Sync>>,
    {
        Self::_new(kind, error.into())
    }

    pub fn other<E>(error: E) -> Self
    where
        E: Into<super::Box<dyn core::error::Error + Send + Sync>>,
    {
        Self::_new(ErrorKind::Other, error.into())
    }

    fn _new(
        kind: ErrorKind,
        error: super::Box<dyn core::error::Error + Send + Sync>,
    ) -> Self {
        Self(ErrorRepr::Custom(CustomError { kind, error }))
    }

    pub fn kind(&self) -> ErrorKind {
        match &self.0 {
            ErrorRepr::Simple(kind) => *kind,
            ErrorRepr::Custom(custom_error) => custom_error.kind,
        }
    }

    pub fn is_interrupted(&self) -> bool {
        match &self.0 {
            ErrorRepr::Simple(kind) => *kind == ErrorKind::Interrupted,
            ErrorRepr::Custom(custom_error) => custom_error.kind == ErrorKind::Interrupted,
        }
    }
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.0 {
            ErrorRepr::Simple(kind) => f.debug_tuple("Kind").field(kind).finish(),
            ErrorRepr::Custom(custom_error) => core::fmt::Debug::fmt(custom_error, f),
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.0 {
            ErrorRepr::Simple(kind) => write!(f, "{}", error_kind_to_str(*kind)),
            ErrorRepr::Custom(custom_error) => custom_error.error.fmt(f),
        }
    }
}

impl From<ReadExactError<Error>> for Error {
    fn from(error: ReadExactError<Error>) -> Self {
        match error {
            ReadExactError::UnexpectedEof => Self::other("unexpected end of file"),
            ReadExactError::Other(err) => err,
        }
    }
}

impl embedded_io::Error for Error {
    fn kind(&self) -> ErrorKind {
        self.kind()
    }
}

fn error_kind_to_str(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::AddrInUse => "address in use",
        ErrorKind::AddrNotAvailable => "address not available",
        ErrorKind::AlreadyExists => "entity already exists",
        ErrorKind::BrokenPipe => "broken pipe",
        ErrorKind::ConnectionAborted => "connection aborted",
        ErrorKind::ConnectionRefused => "connection refused",
        ErrorKind::ConnectionReset => "connection reset",
        ErrorKind::Interrupted => "operation interrupted",
        ErrorKind::InvalidData => "invalid data",
        ErrorKind::InvalidInput => "invalid input parameter",
        ErrorKind::NotConnected => "not connected",
        ErrorKind::NotFound => "entity not found",
        ErrorKind::Other => "other error",
        ErrorKind::OutOfMemory => "out of memory",
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::TimedOut => "timed out",
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::WriteZero => "write zero",
        _ => unimplemented!(),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    pub const fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0, inner }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub const fn get_ref(&self) -> &T {
        &self.inner
    }

    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub const fn position(&self) -> u64 {
        self.pos
    }

    pub const fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}